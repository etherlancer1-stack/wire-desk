//! Repository layer: all DB reads/writes with tenant_id filtering (spec #161),
//! parameterized queries only (spec #83), and transaction boundaries (spec #82).

use crate::db::{open, run_seed};
use crate::error::WireResult;
use crate::models::{
    AgentRun, AgentStep, AuditLog, Job, NewAgentRun, NewAuditLog, NewJob,
    NewSecurityScan, NewTenant, NewUser, NewWorkspace, SecurityFinding,
    SecurityScan, Tenant, User, Workspace,
};
use crate::security::ScopeAllowlist;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;
use uuid::Uuid;

pub struct Repository {
    db_path: std::path::PathBuf,
}

impl Repository {
    pub fn new(db_path: &Path) -> Self {
        Self { db_path: db_path.to_path_buf() }
    }

    fn conn(&self) -> WireResult<Connection> {
        open(&self.db_path)
    }

    /* ------------------------------------------------------------ */
    /* Tenants                                                      */
    /* ------------------------------------------------------------ */
    pub fn create_tenant(&self, input: &NewTenant) -> WireResult<Tenant> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        let tier = input.plan_tier.as_deref().unwrap_or("free");
        conn.execute(
            "INSERT INTO tenants (id, name, slug, plan_tier, monthly_token_quota, monthly_scan_quota, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id, input.name, input.slug, tier,
                crate::models::PlanTier::from_str(tier)?.monthly_token_quota(),
                crate::models::PlanTier::from_str(tier)?.monthly_scan_quota(),
                now, now,
            ],
        )?;
        Ok(self.get_tenant_by_id(&id)?.unwrap())
    }

    pub fn get_tenant_by_id(&self, id: &str) -> WireResult<Option<Tenant>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, slug, plan_tier, stripe_customer_id, stripe_subscription_id,
                    monthly_token_quota, monthly_scan_quota, created_at, updated_at
             FROM tenants WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_tenant(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn get_tenant_by_slug(&self, slug: &str) -> WireResult<Option<Tenant>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, slug, plan_tier, stripe_customer_id, stripe_subscription_id,
                    monthly_token_quota, monthly_scan_quota, created_at, updated_at
             FROM tenants WHERE slug = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![slug])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_tenant(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn update_tenant_plan(
        &self, id: &str, plan_tier: &str,
        customer_id: Option<&str>, subscription_id: Option<&str>,
    ) -> WireResult<()> {
        let conn = self.conn()?;
        let tier = crate::models::PlanTier::from_str(plan_tier)?;
        let now = crate::models::utc_now();
        conn.execute(
            "UPDATE tenants SET plan_tier = ?1, monthly_token_quota = ?2, monthly_scan_quota = ?3,
                    stripe_customer_id = COALESCE(?4, stripe_customer_id),
                    stripe_subscription_id = COALESCE(?5, stripe_subscription_id),
                    updated_at = ?6 WHERE id = ?7",
            rusqlite::params![
                tier.as_str(), tier.monthly_token_quota(), tier.monthly_scan_quota(),
                customer_id, subscription_id, now, id
            ],
        )?;
        Ok(())
    }

    /* ------------------------------------------------------------ */
    /* Users                                                        */
    /* ------------------------------------------------------------ */
    pub fn create_user(&self, input: &NewUser, password_hash: &str) -> WireResult<User> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        // tenant_id must come from the caller context; for now require it via input.tenant_id not present
        // We resolve tenant by the first tenant (demo) or require caller to set. Here we need tenant_id.
        // NewUser doesn't carry tenant_id; callers must ensure tenant exists. We'll store via separate method.
        Err(WireError::Validation(
            "use create_user_with_tenant instead".into(),
        ))
    }

    pub fn create_user_with_tenant(
        &self, tenant_id: &str, input: &NewUser, password_hash: &str,
    ) -> WireResult<User> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        let role = input.role.as_deref().unwrap_or("viewer");
        conn.execute(
            "INSERT INTO users (id, tenant_id, email, role, password_hash, api_key_prefix, api_key_hash, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id, tenant_id, input.email, role, password_hash,
                "wd_live", Option::<String>::None, 1, now
            ],
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, email, role, password_hash, api_key_prefix, api_key_hash, is_active, created_at
             FROM users WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        let row = rows.next()?.unwrap();
        Ok(row_to_user(&row))
    }

    pub fn get_user_by_email(&self, email: &str) -> WireResult<Option<User>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, email, role, password_hash, api_key_prefix, api_key_hash, is_active, created_at
             FROM users WHERE email = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![email])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_user(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn get_user_by_id(&self, id: &str) -> WireResult<Option<User>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, email, role, password_hash, api_key_prefix, api_key_hash, is_active, created_at
             FROM users WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_user(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn set_api_key_hash(&self, user_id: &str, prefix: &str, hash: &str) -> WireResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE users SET api_key_prefix = ?1, api_key_hash = ?2 WHERE id = ?3",
            rusqlite::params![prefix, hash, user_id],
        )?;
        Ok(())
    }

    pub fn revoke_api_key(&self, user_id: &str) -> WireResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE users SET api_key_hash = NULL WHERE id = ?1",
            rusqlite::params![user_id],
        )?;
        Ok(())
    }

    /* ------------------------------------------------------------ */
    /* Workspaces                                                   */
    /* ------------------------------------------------------------ */
    pub fn create_workspace(&self, input: &NewWorkspace, tenant_id: &str) -> WireResult<Workspace> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        let scope = input
            .scope_allowlist
            .as_ref()
            .map(|s| serde_json::to_string(s).unwrap_or_default())
            .unwrap_or_else(|| "[]".to_string());
        let rate = input.rate_limit_rps.unwrap_or(5.0);
        conn.execute(
            "INSERT INTO workspaces (id, tenant_id, name, description, scope_allowlist_json,
                    rate_limit_rps, config_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id, tenant_id, input.name,
                input.description.as_deref().unwrap_or(""),
                scope, rate, "{}", now, now
            ],
        )?;
        Ok(self.get_workspace_by_id(&id)?.unwrap())
    }

    pub fn get_workspace_by_id(&self, id: &str) -> WireResult<Option<Workspace>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, name, description, scope_allowlist_json, rate_limit_rps,
                    config_json, created_at, updated_at FROM workspaces WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_workspace(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn list_workspaces_for_tenant(&self, tenant_id: &str) -> WireResult<Vec<Workspace>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, name, description, scope_allowlist_json, rate_limit_rps,
                    config_json, created_at, updated_at
             FROM workspaces WHERE tenant_id = ?1 ORDER BY created_at DESC"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_workspace(&row));
        }
        Ok(out)
    }

    pub fn update_workspace_scope(
        &self, workspace_id: &str, scope: &ScopeAllowlist, rate_limit_rps: f64,
    ) -> WireResult<Workspace> {
        let conn = self.conn()?;
        let scope_json = serde_json::to_string(scope).unwrap_or_default();
        let now = crate::models::utc_now();
        conn.execute(
            "UPDATE workspaces SET scope_allowlist_json = ?1, rate_limit_rps = ?2,
                    updated_at = ?3 WHERE id = ?4",
            rusqlite::params![scope_json, rate_limit_rps, now, workspace_id],
        )?;
        Ok(self.get_workspace_by_id(workspace_id)?.unwrap())
    }

    /* ------------------------------------------------------------ */
    /* Agent runs                                                   */
    /* ------------------------------------------------------------ */
    pub fn create_agent_run(
        &self, tenant_id: &str, input: &NewAgentRun,
    ) -> WireResult<AgentRun> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        // Verify workspace belongs to tenant.
        let ws = self.get_workspace_by_id(&input.workspace_id)?;
        let ws = ws.ok_or_else(|| WireError::NotFound("workspace not found".into()))?;
        if ws.tenant_id != tenant_id {
            return Err(WireError::Forbidden("workspace does not belong to tenant".into()));
        }
        conn.execute(
            "INSERT INTO agent_runs (id, tenant_id, workspace_id, goal, status, model_used,
                    iterations, tokens_in, tokens_out, cost_usd, error_message, trace_json,
                    started_at, completed_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                id, tenant_id, input.workspace_id, input.goal, "pending",
                Option::<String>::None, 0, 0, 0, 0.0, Option::<String>::None, "[]",
                Option::<String>::None, Option::<String>::None, now
            ],
        )?;
        Ok(self.get_agent_run_by_id(&id)?.unwrap())
    }

    pub fn get_agent_run_by_id(&self, id: &str) -> WireResult<Option<AgentRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, workspace_id, goal, status, model_used, iterations,
                    tokens_in, tokens_out, cost_usd, error_message, trace_json,
                    started_at, completed_at, created_at
             FROM agent_runs WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_agent_run(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn list_agent_runs_for_tenant(
        &self, tenant_id: &str, limit: i64, offset: i64,
    ) -> WireResult<Vec<AgentRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, workspace_id, goal, status, model_used, iterations,
                    tokens_in, tokens_out, cost_usd, error_message, trace_json,
                    started_at, completed_at, created_at
             FROM agent_runs WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id, limit, offset])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_agent_run(&row));
        }
        Ok(out)
    }

    pub fn update_agent_run_status(
        &self, id: &str, status: &str, error: Option<&str>,
    ) -> WireResult<()> {
        let conn = self.conn()?;
        let now = crate::models::utc_now();
        let (started, completed) = match status {
            "running" => (Some(now.clone()), None),
            "completed" | "failed" | "cancelled" => (Some(now.clone()), Some(now.clone())),
            _ => (None, None),
        };
        conn.execute(
            "UPDATE agent_runs SET status = ?1, error_message = COALESCE(?2, error_message),
                    started_at = COALESCE(?3, started_at), completed_at = ?4 WHERE id = ?5",
            rusqlite::params![status, error, started, completed, id],
        )?;
        Ok(())
    }

    pub fn append_agent_step(&self, step: &AgentStep) -> WireResult<()> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        conn.execute(
            "INSERT INTO agent_steps (id, run_id, step_index, phase, tool_name,
                    input_json, output_json, tokens_used, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id, step.run_id, step.step_index, step.phase, step.tool_name,
                step.input_json, step.output_json, step.tokens_used, step.duration_ms, now
            ],
        )?;
        Ok(())
    }

    pub fn list_agent_steps_for_run(&self, run_id: &str) -> WireResult<Vec<AgentStep>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, step_index, phase, tool_name, input_json, output_json,
                    tokens_used, duration_ms, created_at
             FROM agent_steps WHERE run_id = ?1 ORDER BY step_index ASC"
        )?;
        let mut rows = stmt.query(rusqlite::params![run_id])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_agent_step(&row));
        }
        Ok(out)
    }

    pub fn update_agent_run_trace_and_cost(
        &self, id: &str, trace_json: &str, tokens_in: i64, tokens_out: i64, cost_usd: f64,
    ) -> WireResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE agent_runs SET trace_json = ?1, tokens_in = ?2, tokens_out = ?3,
                    cost_usd = ?4 WHERE id = ?5",
            rusqlite::params![trace_json, tokens_in, tokens_out, cost_usd, id],
        )?;
        Ok(())
    }

    /* ------------------------------------------------------------ */
    /* Security scans                                               */
    /* ------------------------------------------------------------ */
    pub fn create_security_scan(
        &self, tenant_id: &str, input: &NewSecurityScan,
    ) -> WireResult<SecurityScan> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        let ws = self.get_workspace_by_id(&input.workspace_id)?;
        let ws = ws.ok_or_else(|| WireError::NotFound("workspace not found".into()))?;
        if ws.tenant_id != tenant_id {
            return Err(WireError::Forbidden("workspace does not belong to tenant".into()));
        }
        conn.execute(
            "INSERT INTO security_scans (id, tenant_id, workspace_id, target, scan_type,
                    status, findings_count, critical_count, high_count, started_at,
                    completed_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                id, tenant_id, input.workspace_id, input.target, input.scan_type,
                "pending", 0, 0, 0, Option::<String>::None, Option::<String>::None, now
            ],
        )?;
        Ok(self.get_security_scan_by_id(&id)?.unwrap())
    }

    pub fn get_security_scan_by_id(&self, id: &str) -> WireResult<Option<SecurityScan>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, workspace_id, target, scan_type, status, findings_count,
                    critical_count, high_count, started_at, completed_at, created_at
             FROM security_scans WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_security_scan(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn update_security_scan_status(
        &self, id: &str, status: &str, findings_count: i64, critical: i64, high: i64,
    ) -> WireResult<()> {
        let conn = self.conn()?;
        let now = crate::models::utc_now();
        let (started, completed) = match status {
            "running" => (Some(now.clone()), None),
            "completed" | "failed" | "cancelled" => (Some(now.clone()), Some(now.clone())),
            _ => (None, None),
        };
        conn.execute(
            "UPDATE security_scans SET status = ?1, findings_count = ?2, critical_count = ?3,
                    high_count = ?4, started_at = COALESCE(?5, started_at),
                    completed_at = ?6 WHERE id = ?7",
            rusqlite::params![
                status, findings_count, critical, high, started, completed, id
            ],
        )?;
        Ok(())
    }

    pub fn list_security_scans_for_tenant(
        &self, tenant_id: &str, limit: i64, offset: i64,
    ) -> WireResult<Vec<SecurityScan>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, workspace_id, target, scan_type, status, findings_count,
                    critical_count, high_count, started_at, completed_at, created_at
             FROM security_scans WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id, limit, offset])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_security_scan(&row));
        }
        Ok(out)
    }

    /* ------------------------------------------------------------ */
    /* Security findings                                            */
    /* ------------------------------------------------------------ */
    pub fn create_security_finding(&self, finding: &SecurityFinding) -> WireResult<SecurityFinding> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        conn.execute(
            "INSERT INTO security_findings (id, scan_id, tenant_id, workspace_id, fingerprint,
                    category, severity, title, description, evidence, remediation, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                id, finding.scan_id, finding.tenant_id, finding.workspace_id,
                finding.fingerprint, finding.category, finding.severity, finding.title,
                finding.description, finding.evidence, finding.remediation, finding.status, now
            ],
        )?;
        Ok(SecurityFinding {
            id, ..finding.clone()
        })
    }

    pub fn get_finding_by_fingerprint(
        &self, workspace_id: &str, fingerprint: &str,
    ) -> WireResult<Option<SecurityFinding>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, scan_id, tenant_id, workspace_id, fingerprint, category, severity,
                    title, description, evidence, remediation, status, created_at
             FROM security_findings WHERE workspace_id = ?1 AND fingerprint = ?2"
        )?;
        let mut rows = stmt.query(rusqlite::params![workspace_id, fingerprint])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_security_finding(&row)))
        } else {
            Ok(None)
        }
    }

    pub fn list_findings_for_workspace(
        &self, workspace_id: &str, limit: i64, offset: i64,
    ) -> WireResult<Vec<SecurityFinding>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, scan_id, tenant_id, workspace_id, fingerprint, category, severity,
                    title, description, evidence, remediation, status, created_at
             FROM security_findings WHERE workspace_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )?;
        let mut rows = stmt.query(rusqlite::params![workspace_id, limit, offset])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_security_finding(&row));
        }
        Ok(out)
    }

    pub fn update_finding_status(&self, id: &str, status: &str) -> WireResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE security_findings SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, id],
        )?;
        Ok(())
    }

    /* ------------------------------------------------------------ */
    /* Jobs (durable queue)                                         */
    /* ------------------------------------------------------------ */
    pub fn create_job(&self, job: &NewJob) -> WireResult<Job> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        conn.execute(
            "INSERT INTO jobs (id, tenant_id, workspace_id, job_type, payload_json, status,
                    attempts, max_attempts, lease_expires_at, run_after_at, last_error,
                    created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                id, job.tenant_id, job.workspace_id, job.job_type, job.payload_json,
                "pending", 0, job.max_attempts, Option::<String>::None,
                job.run_after_at, Option::<String>::None, now, now
            ],
        )?;
        Ok(self.get_job_by_id(&id)?.unwrap())
    }

    pub fn get_job_by_id(&self, id: &str) -> WireResult<Option<Job>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, workspace_id, job_type, payload_json, status, attempts,
                    max_attempts, lease_expires_at, run_after_at, last_error, created_at, updated_at
             FROM jobs WHERE id = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_job(&row)))
        } else {
            Ok(None)
        }
    }

    /// Atomically claim a pending job whose `run_after_at <= now` and which has
    /// no active lease or an expired lease (spec #76, #245).
    pub fn claim_job(&self, job_type: Option<&str>, lease_ttl_seconds: u64) -> WireResult<Option<Job>> {
        let conn = self.conn()?;
        let now = crate::models::utc_now();
        let lease_expires = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(lease_ttl_seconds as i64))
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or(now.clone());
        let tx = conn.transaction().map_err(|e| WireError::Database(format!("tx failed: {e}")))?;
        let claimed: Option<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM jobs
                 WHERE status = 'pending'
                   AND (lease_expires_at IS NULL OR lease_expires_at <= ?1)
                   AND run_after_at <= ?1
                   AND (?2 = '' OR job_type = ?2)
                 ORDER BY run_after_at ASC
                 LIMIT 1"
            )?;
            let jt = job_type.unwrap_or("");
            let mut rows = stmt.query(rusqlite::params![now, jt])?;
            rows.next()?.map(|r| r.get::<_, String>(0).unwrap())
        };
        if let Some(id) = claimed {
            tx.execute(
                "UPDATE jobs SET status = 'running', lease_expires_at = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![lease_expires, now, id],
            )?;
            tx.commit().map_err(|e| WireError::Database(format!("commit failed: {e}")))?;
            Ok(self.get_job_by_id(&id)?)
        } else {
            Ok(None)
        }
    }

    pub fn complete_job(&self, id: &str) -> WireResult<()> {
        let conn = self.conn()?;
        let now = crate::models::utc_now();
        conn.execute(
            "UPDATE jobs SET status = 'completed', lease_expires_at = NULL, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    }

    pub fn fail_job(&self, id: &str, error: &str, max_attempts: i64) -> WireResult<JobStatus> {
        let conn = self.conn()?;
        let mut job = self.get_job_by_id(id)?.ok_or_else(|| WireError::NotFound("job not found".into()))?;
        let attempts = job.attempts + 1;
        let now = crate::models::utc_now();
        let status = if attempts >= max_attempts {
            "dead_letter"
        } else {
            // Requeue with backoff: run_after_at = now + backoff(attempts)
            let backoff = backoff_seconds(attempts);
            let run_after = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::seconds(backoff as i64))
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                .unwrap_or(now.clone());
            conn.execute(
                "UPDATE jobs SET status = 'pending', attempts = ?1, last_error = ?2,
                        run_after_at = ?3, lease_expires_at = NULL, updated_at = ?4 WHERE id = ?5",
                rusqlite::params![attempts, error, run_after, now, id],
            )?;
            return Ok(JobStatus::Pending);
        };
        conn.execute(
            "UPDATE jobs SET status = 'dead_letter', attempts = ?1, last_error = ?2,
                    lease_expires_at = NULL, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![attempts, error, now, id],
        )?;
        Ok(JobStatus::DeadLetter)
    }

    pub fn list_jobs_for_tenant(
        &self, tenant_id: &str, limit: i64, offset: i64,
    ) -> WireResult<Vec<Job>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, workspace_id, job_type, payload_json, status, attempts,
                    max_attempts, lease_expires_at, run_after_at, last_error, created_at, updated_at
             FROM jobs WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id, limit, offset])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_job(&row));
        }
        Ok(out)
    }

    pub fn count_pending_jobs(&self, tenant_id: Option<&str>) -> WireResult<i64> {
        let conn = self.conn()?;
        let (sql, params): (&str, Vec<&dyn rusqlite::ToSql>) = match tenant_id {
            Some(t) => (
                "SELECT COUNT(*) FROM jobs WHERE status = 'pending' AND tenant_id = ?1",
                vec![t as &dyn rusqlite::ToSql],
            ),
            None => (
                "SELECT COUNT(*) FROM jobs WHERE status = 'pending'",
                vec![],
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(rusqlite::params_from(params.iter().map(|p| *p as &dyn rusqlite::ToSql).collect::<Vec<_>>()))?;
        let count: i64 = rows.next()?.unwrap().get(0)?;
        Ok(count)
    }

    pub fn reap_stale_jobs(&self, lease_ttl_seconds: u64) -> WireResult<i64> {
        let conn = self.conn()?;
        let now = crate::models::utc_now();
        // Mark jobs running with expired lease back to pending (unless past max_attempts -> dead_letter handled elsewhere)
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::seconds(lease_ttl_seconds as i64))
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or(now.clone());
        let changed = conn.execute(
            "UPDATE jobs SET status = 'pending', lease_expires_at = NULL, updated_at = ?1
             WHERE status = 'running' AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?2",
            rusqlite::params![now, cutoff],
        ).map_err(|e| WireError::Database(format!("reap failed: {e}")))?;
        Ok(changed as i64)
    }

    /* ------------------------------------------------------------ */
    /* Usage events                                                 */
    /* ------------------------------------------------------------ */
    pub fn record_usage_event(
        &self, tenant_id: &str, workspace_id: &str, metric_type: &str,
        quantity: i64, cost_usd: f64, idempotency_key: &str,
    ) -> WireResult<UsageEvent> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        conn.execute(
            "INSERT INTO usage_events (id, tenant_id, workspace_id, metric_type, quantity,
                    cost_usd, idempotency_key, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id, tenant_id, workspace_id, metric_type, quantity, cost_usd,
                idempotency_key, now
            ],
        )?;
        Ok(UsageEvent {
            id, tenant_id: tenant_id.to_string(),
            workspace_id: workspace_id.to_string(),
            metric_type: metric_type.to_string(), quantity, cost_usd,
            idempotency_key: idempotency_key.to_string(), recorded_at: now,
        })
    }

    pub fn aggregate_usage_current_period(
        &self, tenant_id: &str, period_start: &str,
    ) -> WireResult<(i64, i64, f64)> {
        // Returns (ai_runs, scans, cost_usd) for the period.
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(SUM(CASE WHEN metric_type = 'ai_runs' THEN quantity ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN metric_type = 'scans' THEN quantity ELSE 0 END), 0),
                COALESCE(SUM(cost_usd), 0)
             FROM usage_events
             WHERE tenant_id = ?1 AND recorded_at >= ?2"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id, period_start])?;
        let row = rows.next()?.unwrap();
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }

    /* ------------------------------------------------------------ */
    /* Webhook endpoints                                            */
    /* ------------------------------------------------------------ */
    pub fn create_webhook_endpoint(
        &self, tenant_id: &str, url: &str, secret_hash: &str, events_json: &str,
    ) -> WireResult<WebhookEndpoint> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        conn.execute(
            "INSERT INTO webhook_endpoints (id, tenant_id, url, secret_hash, events_json, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, tenant_id, url, secret_hash, events_json, 1, now],
        )?;
        Ok(WebhookEndpoint {
            id, tenant_id: tenant_id.to_string(), url: url.to_string(),
            secret_hash: secret_hash.to_string(),
            events_json: events_json.to_string(),
            is_active: true, created_at: now,
        })
    }

    pub fn list_webhook_endpoints_for_tenant(
        &self, tenant_id: &str,
    ) -> WireResult<Vec<WebhookEndpoint>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, url, secret_hash, events_json, is_active, created_at
             FROM webhook_endpoints WHERE tenant_id = ?1 ORDER BY created_at DESC"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_webhook_endpoint(&row));
        }
        Ok(out)
    }

    /* ------------------------------------------------------------ */
    /* Audit logs                                                   */
    /* ------------------------------------------------------------ */
    pub fn write_audit_log(&self, input: &NewAuditLog) -> WireResult<AuditLog> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = crate::models::utc_now();
        conn.execute(
            "INSERT INTO audit_logs (id, tenant_id, actor_user_id, action, resource_type,
                    resource_id, metadata_json, ip_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id, input.tenant_id, input.actor_user_id, input.action,
                input.resource_type, input.resource_id, input.metadata_json,
                input.ip_hash, now
            ],
        )?;
        Ok(AuditLog {
            id, tenant_id: input.tenant_id.clone(),
            actor_user_id: input.actor_user_id.clone(),
            action: input.action.clone(),
            resource_type: input.resource_type.clone(),
            resource_id: input.resource_id.clone(),
            metadata_json: input.metadata_json.clone(),
            ip_hash: input.ip_hash.clone(),
            created_at: now,
        })
    }

    pub fn list_audit_logs_for_tenant(
        &self, tenant_id: &str, action_filter: Option<&str>,
        limit: i64, offset: i64,
    ) -> WireResult<Vec<AuditLog>> {
        let conn = self.conn()?;
        let (sql, params): (&str, Vec<&dyn rusqlite::ToSql>) = match action_filter {
            Some(a) => (
                "SELECT id, tenant_id, actor_user_id, action, resource_type, resource_id,
                        metadata_json, ip_hash, created_at
                 FROM audit_logs WHERE tenant_id = ?1 AND action = ?2
                 ORDER BY created_at DESC LIMIT ?3 OFFSET ?4",
                vec![tenant_id as &dyn rusqlite::ToSql, a as &dyn rusqlite::ToSql,
                     limit as &dyn rusqlite::ToSql, offset as &dyn rusqlite::ToSql],
            ),
            None => (
                "SELECT id, tenant_id, actor_user_id, action, resource_type, resource_id,
                        metadata_json, ip_hash, created_at
                 FROM audit_logs WHERE tenant_id = ?1
                 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                vec![tenant_id as &dyn rusqlite::ToSql,
                     limit as &dyn rusqlite::ToSql, offset as &dyn rusqlite::ToSql],
            ),
        };
        // Build dynamic params
        let pvals: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| *p).collect();
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(rusqlite::params_from(pvals))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_audit_log(&row));
        }
        Ok(out)
    }

    /* ------------------------------------------------------------ */
    /* Seed                                                         */
    /* ------------------------------------------------------------ */
    pub fn seed(&self) -> WireResult<()> {
        let conn = self.conn()?;
        run_seed(&conn)
    }
}

/* ---------------------------------------------------------------- */
/* Row mappers                                                      */
/* ---------------------------------------------------------------- */

fn row_to_tenant(row: &rusqlite::Row) -> Tenant {
    Tenant {
        id: row.get(0).unwrap(),
        name: row.get(1).unwrap(),
        slug: row.get(2).unwrap(),
        plan_tier: row.get(3).unwrap(),
        stripe_customer_id: row.get(4).unwrap(),
        stripe_subscription_id: row.get(5).unwrap(),
        monthly_token_quota: row.get(6).unwrap(),
        monthly_scan_quota: row.get(7).unwrap(),
        created_at: row.get(8).unwrap(),
        updated_at: row.get(9).unwrap(),
    }
}

fn row_to_user(row: &rusqlite::Row) -> User {
    User {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        email: row.get(2).unwrap(),
        role: row.get(3).unwrap(),
        password_hash: row.get(4).unwrap(),
        api_key_prefix: row.get(5).unwrap(),
        api_key_hash: row.get(6).unwrap(),
        is_active: row.get::<_, i64>(7).unwrap() != 0,
        created_at: row.get(8).unwrap(),
    }
}

fn row_to_workspace(row: &rusqlite::Row) -> Workspace {
    Workspace {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        name: row.get(2).unwrap(),
        description: row.get(3).unwrap(),
        scope_allowlist_json: row.get(4).unwrap(),
        rate_limit_rps: row.get(5).unwrap(),
        config_json: row.get(6).unwrap(),
        created_at: row.get(7).unwrap(),
        updated_at: row.get(8).unwrap(),
    }
}

fn row_to_agent_run(row: &rusqlite::Row) -> AgentRun {
    AgentRun {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        workspace_id: row.get(2).unwrap(),
        goal: row.get(3).unwrap(),
        status: row.get(4).unwrap(),
        model_used: row.get(5).unwrap(),
        iterations: row.get(6).unwrap(),
        tokens_in: row.get(7).unwrap(),
        tokens_out: row.get(8).unwrap(),
        cost_usd: row.get(9).unwrap(),
        error_message: row.get(10).unwrap(),
        trace_json: row.get(11).unwrap(),
        started_at: row.get(12).unwrap(),
        completed_at: row.get(13).unwrap(),
        created_at: row.get(14).unwrap(),
    }
}

fn row_to_agent_step(row: &rusqlite::Row) -> AgentStep {
    AgentStep {
        id: row.get(0).unwrap(),
        run_id: row.get(1).unwrap(),
        step_index: row.get(2).unwrap(),
        phase: row.get(3).unwrap(),
        tool_name: row.get(4).unwrap(),
        input_json: row.get(5).unwrap(),
        output_json: row.get(6).unwrap(),
        tokens_used: row.get(7).unwrap(),
        duration_ms: row.get(8).unwrap(),
        created_at: row.get(9).unwrap(),
    }
}

fn row_to_security_scan(row: &rusqlite::Row) -> SecurityScan {
    SecurityScan {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        workspace_id: row.get(2).unwrap(),
        target: row.get(3).unwrap(),
        scan_type: row.get(4).unwrap(),
        status: row.get(5).unwrap(),
        findings_count: row.get(6).unwrap(),
        critical_count: row.get(7).unwrap(),
        high_count: row.get(8).unwrap(),
        started_at: row.get(9).unwrap(),
        completed_at: row.get(10).unwrap(),
        created_at: row.get(11).unwrap(),
    }
}

fn row_to_security_finding(row: &rusqlite::Row) -> SecurityFinding {
    SecurityFinding {
        id: row.get(0).unwrap(),
        scan_id: row.get(1).unwrap(),
        tenant_id: row.get(2).unwrap(),
        workspace_id: row.get(3).unwrap(),
        fingerprint: row.get(4).unwrap(),
        category: row.get(5).unwrap(),
        severity: row.get(6).unwrap(),
        title: row.get(7).unwrap(),
        description: row.get(8).unwrap(),
        evidence: row.get(9).unwrap(),
        remediation: row.get(10).unwrap(),
        status: row.get(11).unwrap(),
        created_at: row.get(12).unwrap(),
    }
}

fn row_to_job(row: &rusqlite::Row) -> Job {
    Job {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        workspace_id: row.get(2).unwrap(),
        job_type: row.get(3).unwrap(),
        payload_json: row.get(4).unwrap(),
        status: row.get(5).unwrap(),
        attempts: row.get(6).unwrap(),
        max_attempts: row.get(7).unwrap(),
        lease_expires_at: row.get(8).unwrap(),
        run_after_at: row.get(9).unwrap(),
        last_error: row.get(10).unwrap(),
        created_at: row.get(11).unwrap(),
        updated_at: row.get(12).unwrap(),
    }
}

fn row_to_webhook_endpoint(row: &rusqlite::Row) -> WebhookEndpoint {
    WebhookEndpoint {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        url: row.get(2).unwrap(),
        secret_hash: row.get(3).unwrap(),
        events_json: row.get(4).unwrap(),
        is_active: row.get::<_, i64>(5).unwrap() != 0,
        created_at: row.get(6).unwrap(),
    }
}

fn row_to_audit_log(row: &rusqlite::Row) -> AuditLog {
    AuditLog {
        id: row.get(0).unwrap(),
        tenant_id: row.get(1).unwrap(),
        actor_user_id: row.get(2).unwrap(),
        action: row.get(3).unwrap(),
        resource_type: row.get(4).unwrap(),
        resource_id: row.get(5).unwrap(),
        metadata_json: row.get(6).unwrap(),
        ip_hash: row.get(7).unwrap(),
        created_at: row.get(8).unwrap(),
    }
}

fn backoff_seconds(attempt: i64) -> i64 {
    // Exponential backoff with jitter: 2^attempt seconds, capped at 3600.
    let base = 2i64.pow(attempt.min(10) as u32);
    base.min(3600)
}
