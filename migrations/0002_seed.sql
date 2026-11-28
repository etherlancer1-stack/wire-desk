-- Wire Desk — idempotent seed data (safe to re-run).
-- Creates a demo tenant, workspaces, sample findings, and a sample agent run for local testing.
-- All writes use INSERT OR IGNORE / idempotent keys so repeated startup never fails.

PRAGMA foreign_keys = ON;

-- Demo tenant (free tier).
INSERT OR IGNORE INTO tenants (id, name, slug, plan_tier, monthly_token_quota, monthly_scan_quota, created_at, updated_at)
VALUES ('tenant_demo_000000000001', 'Demo Tenant', 'demo', 'free', 50000, 100, datetime('now'), datetime('now'));

-- Demo owner user (password hash is Argon2id placeholder; real hash set via CLI/API).
INSERT OR IGNORE INTO users (id, tenant_id, email, role, password_hash, api_key_prefix, api_key_hash, is_active, created_at)
VALUES ('user_demo_000000000001', 'tenant_demo_000000000001', 'owner@demo.local', 'owner', '', 'wd_live', '', 1, datetime('now'));

-- Demo workspace with a narrow scope allowlist (example.com only).
INSERT OR IGNORE INTO workspaces (id, tenant_id, name, description, scope_allowlist_json, rate_limit_rps, config_json, created_at, updated_at)
VALUES (
  'workspace_demo_000000000001',
  'tenant_demo_000000000001',
  'Demo Workspace',
  'Starter workspace for local smoke tests.',
  '{"domains":["example.com"],"wildcard_domains":["*.example.com"],"cidrs":[],"ips":[],"paths":[],"allow_private_ips":false}',
  5.0,
  '{"timezone":"UTC","language":"en"}',
  datetime('now'),
  datetime('now')
);

-- Sample security scan (completed).
INSERT OR IGNORE INTO security_scans (id, tenant_id, workspace_id, target, scan_type, status, findings_count, critical_count, high_count, started_at, completed_at, created_at)
VALUES ('scan_demo_000000000001', 'tenant_demo_000000000001', 'workspace_demo_000000000001', 'https://example.com', 'passive_recon', 'completed', 2, 0, 1, datetime('now'), datetime('now'), datetime('now'));

-- Sample findings (deduplicated by fingerprint).
INSERT OR IGNORE INTO security_findings (id, scan_id, tenant_id, workspace_id, fingerprint, category, severity, title, description, evidence, remediation, status, created_at)
VALUES (
  'finding_demo_000000000001',
  'scan_demo_000000000001',
  'tenant_demo_000000000001',
  'workspace_demo_000000000001',
  'sha256:demo_finding_headers_001',
  'security_misconfiguration',
  'medium',
  'Missing Content-Security-Policy header',
  'The target does not send a Content-Security-Policy header, increasing exposure to XSS and data-injection attacks.',
  'HTTP response to https://example.com omitted Content-Security-Policy.',
  'Add a restrictive Content-Security-Policy header (e.g. default-src \'self\';).',
  'open',
  datetime('now')
);

INSERT OR IGNORE INTO security_findings (id, scan_id, tenant_id, workspace_id, fingerprint, category, severity, title, description, evidence, remediation, status, created_at)
VALUES (
  'finding_demo_000000000002',
  'scan_demo_000000000001',
  'tenant_demo_000000000001',
  'workspace_demo_000000000001',
  'sha256:demo_finding_hsts_002',
  'security_misconfiguration',
  'low',
  'Missing Strict-Transport-Security header',
  'HTTPS is available but Strict-Transport-Security is not enforced, allowing protocol downgrade exposure.',
  'HTTP response to https://example.com omitted Strict-Transport-Security.',
  'Enable HSTS with a suitable max-age and includeSubDomains.',
  'open',
  datetime('now')
);

-- Update scan finding counts to match seeded findings.
UPDATE security_scans
SET findings_count = 2, critical_count = 0, high_count = 1
WHERE id = 'scan_demo_000000000001';

-- Sample agent run (completed) with a structured trace.
INSERT OR IGNORE INTO agent_runs (id, tenant_id, workspace_id, goal, status, model_used, iterations, tokens_in, tokens_out, cost_usd, error_message, trace_json, started_at, completed_at, created_at)
VALUES (
  'run_demo_000000000001',
  'tenant_demo_000000000001',
  'workspace_demo_000000000001',
  'Summarize the public security posture of example.com and list missing headers.',
  'completed',
  'mock://offline',
  3,
  240,
  180,
  0.0004,
  NULL,
  '[
    {"step_index":0,"phase":"plan","tool_name":null,"input_json":"{}","output_json":"{\"plan\":[\"fetch headers\",\"run SAST\"]}","tokens_used":40,"duration_ms":120},
    {"step_index":1,"phase":"tool_call","tool_name":"http_headers","input_json":"{\"url\":\"https://example.com\"}","output_json":"{\"headers\":{}}","tokens_used":0,"duration_ms":450},
    {"step_index":2,"phase":"reflection","tool_name":null,"input_json":"{}","output_json":"{\"conclusion\":\"missing CSP and HSTS\"}","tokens_used":60,"duration_ms":200}
  ]',
  datetime('now'),
  datetime('now'),
  datetime('now')
);

-- Sample usage event (idempotent key).
INSERT OR IGNORE INTO usage_events (id, tenant_id, workspace_id, metric_type, quantity, cost_usd, idempotency_key, recorded_at)
VALUES ('usage_demo_000000000001', 'tenant_demo_000000000001', 'workspace_demo_000000000001', 'ai_runs', 1, 0.0004, 'seed_ai_run_001', datetime('now'));

-- Sample audit log entries.
INSERT OR IGNORE INTO audit_logs (id, tenant_id, actor_user_id, action, resource_type, resource_id, metadata_json, ip_hash, created_at)
VALUES ('audit_demo_000000000001', 'tenant_demo_000000000001', 'user_demo_000000000001', 'workspace.created', 'workspace', 'workspace_demo_000000000001', '{"name":"Demo Workspace"}', 'sha256:demo_ip', datetime('now'));

INSERT OR IGNORE INTO audit_logs (id, tenant_id, actor_user_id, action, resource_type, resource_id, metadata_json, ip_hash, created_at)
VALUES ('audit_demo_000000000002', 'tenant_demo_000000000001', 'user_demo_000000000001', 'scan.completed', 'security_scan', 'scan_demo_000000000001', '{"target":"https://example.com","findings":2}', 'sha256:demo_ip', datetime('now'));
