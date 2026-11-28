//! Durable job queue built on SQLite leasing (specs #76-79).
//! Coordinates with `worker.rs` to process agent runs, security scans,
//! webhook deliveries, and report exports.

use crate::error::WireResult;
use crate::models::{Job, JobStatus, JobType};
use crate::repository::Repository;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub struct Queue {
    repo: Repository,
    lease_ttl_seconds: u64,
}

impl Queue {
    pub fn new(db_path: &Path, lease_ttl_seconds: u64) -> Self {
        Self {
            repo: Repository::new(db_path),
            lease_ttl_seconds,
        }
    }

    /// Enqueue a new job (idempotent via caller-supplied idempotency handled at
    /// a higher layer; the job row itself is always new).
    pub fn enqueue(
        &self, tenant_id: &str, workspace_id: &str, job_type: JobType,
        payload: Value, max_attempts: i64, run_after_at: &str,
    ) -> WireResult<Job> {
        self.repo.create_job(&crate::models::NewJob {
            tenant_id: tenant_id.to_string(),
            workspace_id: workspace_id.to_string(),
            job_type: job_type.as_str().to_string(),
            payload_json: Some(payload.to_string()),
            max_attempts: Some(max_attempts),
            run_after_at: run_after_at.to_string(),
        })
    }

    /// Claim the next due job (across all tenants or a specific tenant).
    pub fn claim(&self, job_type: Option<&JobType>) -> WireResult<Option<Job>> {
        let jt = job_type.map(|jt| jt.as_str().to_string());
        self.repo.claim_job(jt.as_deref(), self.lease_ttl_seconds)
    }

    /// Mark a job completed.
    pub fn complete(&self, id: &str) -> WireResult<()> {
        self.repo.complete_job(id)
    }

    /// Mark a job failed; requeue with backoff or dead-letter (spec #77).
    pub fn fail(&self, id: &str, error: &str, max_attempts: i64) -> WireResult<JobStatus> {
        self.repo.fail_job(id, error, max_attempts)
    }

    /// Reap stale jobs whose lease has expired (spec #265).
    pub fn reap_stale(&self) -> WireResult<i64> {
        self.repo.reap_stale_jobs(self.lease_ttl_seconds)
    }

    /// Count pending jobs, optionally per tenant (spec #78 backpressure).
    pub fn count_pending(&self, tenant_id: Option<&str>) -> WireResult<i64> {
        self.repo.count_pending_jobs(tenant_id)
    }

    /// List jobs for a tenant (admin API).
    pub fn list_for_tenant(
        &self, tenant_id: &str, limit: i64, offset: i64,
    ) -> WireResult<Vec<Job>> {
        self.repo.list_jobs_for_tenant(tenant_id, limit, offset)
    }

    pub fn lease_ttl_seconds(&self) -> u64 {
        self.lease_ttl_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_enqueue_and_claim() {
        let dir = temp_dir().join(format!("wired-queue-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.db");
        let queue = Queue::new(&db, 300);
        let job = queue.enqueue(
            "tenant_1", "workspace_1", JobType::SecurityScan,
            serde_json::json!({"target": "https://example.com"}), 3, "2000-01-01T00:00:00Z",
        ).unwrap();
        assert_eq!(job.status, "pending");
        let claimed = queue.claim(None).unwrap().unwrap();
        assert_eq!(claimed.id, job.id);
        assert_eq!(claimed.status, "running");
        queue.complete(&claimed.id).unwrap();
        let after = queue.reap_stale().unwrap();
        assert_eq!(after, 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
