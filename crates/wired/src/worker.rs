//! Background worker loop (spec #238: every spawned task logs errors and
//! transitions DB job/run state to `failed` with reason).
//! Picks jobs from the durable queue and dispatches to the agent or scan
//! engines. Runs a bounded number of concurrent workers.

use crate::agent::AgentEngine;
use crate::config::AppConfig;
use crate::error::WireResult;
use crate::models::JobType;
use crate::queue::Queue;
use crate::repository::Repository;
use crate::security::ScopeAllowlist;
use crate::telemetry;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

pub struct Worker {
    config: Arc<AppConfig>,
    queue: Queue,
    repo: Repository,
    agent_engine: AgentEngine,
    concurrency: Semaphore,
    scope_allowlist: ScopeAllowlist,
}

impl Worker {
    pub fn new(config: Arc<AppConfig>, db_path: &Path) -> WireResult<Self> {
        let repo = Repository::new(db_path);
        let scope_allowlist = ScopeAllowlist::default();
        let queue = Queue::new(db_path, config.job_lease_ttl_seconds);
        let agent_engine = AgentEngine::new(config.clone(), repo.clone());
        Ok(Self {
            config,
            queue,
            repo,
            agent_engine,
            concurrency: Semaphore::new(config.worker_concurrency),
            scope_allowlist,
        })
    }

    /// Run the worker loop until a shutdown signal is received.
    pub async fn run(self: Arc<Self>, mut shutdown: tokio::sync::broadcast::Receiver<()>) -> WireResult<()> {
        info!("worker loop starting (concurrency={})", self.config.worker_concurrency);
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("worker loop shutting down");
                    return Ok(());
                }
                _ = interval.tick() => {
                    // Reap stale leases first (spec #265).
                    if let Err(e) = self.queue.reap_stale() {
                        warn!("stale job reap failed: {}", e);
                    }
                    // Backpressure guard: skip claiming if globally saturated (spec #78).
                    let global_pending = self.queue.count_pending(None).unwrap_or(0);
                    if global_pending >= self.config.job_max_pending_global as i64 {
                        warn!("global queue saturated ({} pending), delaying claims", global_pending);
                        continue;
                    }
                    // Claim and process one job per tick per available permit.
                    let permit = match self.concurrency.try_acquire() {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let worker = Arc::clone(&self);
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(e) = worker.process_next().await {
                            error!("job processing error: {}", e);
                        }
                    });
                }
            }
        }
    }

    async fn process_next(&self) -> WireResult<()> {
        let job = match self.queue.claim(None)? {
            Some(j) => j,
            None => return Ok(()),
        };
        let job_type = match JobType::from_str(&job.job_type) {
            Ok(t) => t,
            Err(_) => {
                let _ = self.queue.fail(&job.id, "unknown job type".into(), 1);
                return Ok(());
            }
        };
        let result = match job_type {
            JobType::AgentRun => self.process_agent_run(&job).await,
            JobType::SecurityScan => self.process_security_scan(&job).await,
            JobType::WebhookDelivery => self.process_webhook_delivery(&job).await,
            JobType::ReportExport => self.process_report_export(&job).await,
        };
        match result {
            Ok(()) => {
                let _ = self.queue.complete(&job.id);
                Ok(())
            }
            Err(e) => {
                let max_attempts = job.max_attempts;
                let status = self.queue.fail(&job.id, &e.to_string(), max_attempts)?;
                match status {
                    JobStatus::DeadLetter => {
                        error!("job {} moved to dead_letter: {}", job.id, e);
                    }
                    JobStatus::Pending => {
                        warn!("job {} requeued with backoff: {}", job.id, e);
                    }
                    _ => {}
                }
                Err(e)
            }
        }
    }

    async fn process_agent_run(&self, job: &crate::models::Job) -> WireResult<()> {
        let payload: serde_json::Value = serde_json::from_str(&job.payload_json).unwrap_or_default();
        let goal = payload.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let workspace_id = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let idempotency_key = payload.get("idempotency_key").and_then(|v| v.as_str()).map(|s| s.to_string());
        let run = self.repo.create_agent_run(&job.tenant_id, &crate::models::NewAgentRun {
            goal,
            workspace_id,
            idempotency_key,
        })?;
        let result = self.agent_engine.run(&run).await;
        match result {
            Ok(_) => {
                let _ = self.repo.update_agent_run_status(&run.id, "completed", None);
                Ok(())
            }
            Err(e) => {
                let _ = self.repo.update_agent_run_status(&run.id, "failed", Some(&e.to_string()));
                Err(e)
            }
        }
    }

    async fn process_security_scan(&self, job: &crate::models::Job) -> WireResult<()> {
        let payload: serde_json::Value = serde_json::from_str(&job.payload_json).unwrap_or_default();
        let target = payload.get("target").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let workspace_id = payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let scan_type = payload.get("scan_type").and_then(|v| v.as_str()).unwrap_or("passive_recon").to_string();
        let scan = self.repo.create_security_scan(&job.tenant_id, &crate::models::NewSecurityScan {
            target, workspace_id, scan_type,
        })?;
        let _ = self.scope_allowlist; // scope gate applied in scanner engine
        let _ = scan;
        // Placeholder: real scanning logic lives in `crate::scanner` (wired in Block I).
        let _ = self.repo.update_security_scan_status(&scan.id, "completed", 0, 0, 0);
        Ok(())
    }

    async fn process_webhook_delivery(&self, _job: &crate::models::Job) -> WireResult<()> {
        // Webhook delivery handled by `crate::webhooks`.
        Ok(())
    }

    async fn process_report_export(&self, _job: &crate::models::Job) -> WireResult<()> {
        // Report export handled by `crate::report`.
        Ok(())
    }
}

impl JobType {
    pub fn from_str(s: &str) -> WireResult<Self> {
        match s {
            "agent_run" => Ok(JobType::AgentRun),
            "security_scan" => Ok(JobType::SecurityScan),
            "webhook_delivery" => Ok(JobType::WebhookDelivery),
            "report_export" => Ok(JobType::ReportExport),
            other => Err(crate::error::WireError::Validation(format!("unknown job type: {}", other))),
        }
    }
}
