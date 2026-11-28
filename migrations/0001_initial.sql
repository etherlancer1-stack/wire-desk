-- Wire Desk — initial schema (SQLite, idempotent migration runner)
-- Applied on every startup via the embedded migration runner.
-- All timestamps stored as ISO-8601 UTC TEXT (YYYY-MM-DDTHH:MM:SSZ).
-- Columns use snake_case per repository naming convention.

PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA temp_store = MEMORY;

-- Migration tracking (idempotent: repeated startup never fails).
CREATE TABLE IF NOT EXISTS _migrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);

-- Tenants (multi-tenant root; every owned record carries tenant_id).
CREATE TABLE IF NOT EXISTS tenants (
    id TEXT PRIMARY KEY,                       -- UUID
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    plan_tier TEXT NOT NULL DEFAULT 'free' CHECK (plan_tier IN ('free','pro','team','enterprise')),
    stripe_customer_id TEXT,
    stripe_subscription_id TEXT,
    monthly_token_quota INTEGER NOT NULL DEFAULT 50000,
    monthly_scan_quota INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tenants_slug ON tenants(slug);
CREATE INDEX IF NOT EXISTS idx_tenants_plan ON tenants(plan_tier);

-- Users (belong to a tenant; RBAC roles).
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'viewer' CHECK (role IN ('owner','admin','analyst','viewer')),
    password_hash TEXT NOT NULL,
    api_key_prefix TEXT NOT NULL DEFAULT 'wd_live',
    api_key_hash TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_tenant ON users(tenant_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_unique ON users(email) WHERE email IS NOT NULL;

-- Workspaces (isolated per tenant; independent scope allowlists, quotas, config).
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT DEFAULT '',
    scope_allowlist_json TEXT NOT NULL DEFAULT '[]',
    rate_limit_rps REAL NOT NULL DEFAULT 5.0,
    config_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workspaces_tenant ON workspaces(tenant_id);

-- Agent runs (multi-agent orchestration records).
CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    goal TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','running','waiting_approval','completed','failed','cancelled')),
    model_used TEXT,
    iterations INTEGER NOT NULL DEFAULT 0,
    tokens_in INTEGER NOT NULL DEFAULT 0,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0.0,
    error_message TEXT,
    trace_json TEXT NOT NULL DEFAULT '[]',
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_runs_tenant ON agent_runs(tenant_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agent_runs_status ON agent_runs(status, created_at);
CREATE INDEX IF NOT EXISTS idx_agent_runs_workspace ON agent_runs(workspace_id);

-- Agent steps (step-by-step trace within a run).
CREATE TABLE IF NOT EXISTS agent_steps (
    id TEXT PRIMARY KEY,                       -- UUID
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('plan','tool_call','tool_result','reflection','final')),
    tool_name TEXT,
    input_json TEXT NOT NULL DEFAULT '{}',
    output_json TEXT NOT NULL DEFAULT '{}',
    tokens_used INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_steps_run ON agent_steps(run_id, step_index);

-- Security scans.
CREATE TABLE IF NOT EXISTS security_scans (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    target TEXT NOT NULL,
    scan_type TEXT NOT NULL CHECK (scan_type IN ('passive_recon','dns_tls_audit','sast_repo','dep_audit','ssrf_check')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','running','completed','failed','cancelled')),
    findings_count INTEGER NOT NULL DEFAULT 0,
    critical_count INTEGER NOT NULL DEFAULT 0,
    high_count INTEGER NOT NULL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_security_scans_tenant ON security_scans(tenant_id, created_at);
CREATE INDEX IF NOT EXISTS idx_security_scans_status ON security_scans(status, created_at);

-- Security findings (deduplicated by fingerprint).
CREATE TABLE IF NOT EXISTS security_findings (
    id TEXT PRIMARY KEY,                       -- UUID
    scan_id TEXT NOT NULL REFERENCES security_scans(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,                 -- SHA-256 of (workspace_id, category, target, title/location)
    category TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('critical','high','medium','low','info')),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    evidence TEXT NOT NULL DEFAULT '',
    remediation TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','triaged','resolved','false_positive')),
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_security_findings_fingerprint ON security_findings(workspace_id, fingerprint);
CREATE INDEX IF NOT EXISTS idx_security_findings_scan ON security_findings(scan_id);
CREATE INDEX IF NOT EXISTS idx_security_findings_severity ON security_findings(tenant_id, severity, created_at);

-- Jobs (durable job queue with leasing).
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    job_type TEXT NOT NULL CHECK (job_type IN ('agent_run','security_scan','webhook_delivery','report_export')),
    payload_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','running','completed','failed','dead_letter')),
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    lease_expires_at TEXT,
    run_after_at TEXT NOT NULL,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_pending ON jobs(status, run_after_at);
CREATE INDEX IF NOT EXISTS idx_jobs_tenant ON jobs(tenant_id, created_at);

-- Usage events (metering for billing & quotas).
CREATE TABLE IF NOT EXISTS usage_events (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    metric_type TEXT NOT NULL CHECK (metric_type IN ('ai_tokens','ai_runs','scans','sandbox_sec','api_calls')),
    quantity INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0.0,
    idempotency_key TEXT NOT NULL UNIQUE,
    recorded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_usage_events_tenant ON usage_events(tenant_id, recorded_at);
CREATE INDEX IF NOT EXISTS idx_usage_events_idempotency ON usage_events(idempotency_key);

-- Webhook endpoints (outbound signed deliveries).
CREATE TABLE IF NOT EXISTS webhook_endpoints (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    secret_hash TEXT NOT NULL,
    events_json TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_endpoints_tenant ON webhook_endpoints(tenant_id);

-- Audit logs (immutable, chronological).
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,                       -- UUID
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    ip_hash TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_tenant ON audit_logs(tenant_id, created_at);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action, created_at);

-- Record this migration.
INSERT OR IGNORE INTO _migrations (version, name, applied_at)
VALUES ('0001', 'initial', datetime('now'));
