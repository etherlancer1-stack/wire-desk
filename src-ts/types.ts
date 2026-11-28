/**
 * Wire Desk — shared TypeScript types.
 * Mirror the Rust models (crates/wired/src/models.rs) and the SQLite schema
 * (migrations/0001_initial.sql / src-ts/db/schema.ts) exactly.
 */

export type PlanTier = "free" | "pro" | "team" | "enterprise";
export type Role = "owner" | "admin" | "analyst" | "viewer";
export type RunStatus = "pending" | "running" | "waiting_approval" | "completed" | "failed" | "cancelled";
export type JobStatus = "pending" | "running" | "completed" | "failed" | "dead_letter";
export type JobType = "agent_run" | "security_scan" | "webhook_delivery" | "report_export";
export type ScanType = "passive_recon" | "dns_tls_audit" | "sast_repo" | "dep_audit" | "ssrf_check";
export type Severity = "critical" | "high" | "medium" | "low" | "info";
export type FindingStatus = "open" | "triaged" | "resolved" | "false_positive";
export type MetricType = "ai_tokens" | "ai_runs" | "scans" | "sandbox_sec" | "api_calls";
export type StepPhase = "plan" | "tool_call" | "tool_result" | "reflection" | "final";

export interface Tenant {
  id: string;
  name: string;
  slug: string;
  plan_tier: PlanTier;
  stripe_customer_id?: string | null;
  stripe_subscription_id?: string | null;
  monthly_token_quota: number;
  monthly_scan_quota: number;
  created_at: string;
  updated_at: string;
}

export interface User {
  id: string;
  tenant_id: string;
  email: string;
  role: Role;
  password_hash: string;
  api_key_prefix: string;
  api_key_hash?: string | null;
  is_active: boolean;
  created_at: string;
}

export interface Workspace {
  id: string;
  tenant_id: string;
  name: string;
  description: string;
  scope_allowlist_json: string;
  rate_limit_rps: number;
  config_json: string;
  created_at: string;
  updated_at: string;
}

export interface ScopeAllowlist {
  domains: string[];
  wildcard_domains: string[];
  cidrs: string[];
  ips: string[];
  paths: string[];
  allow_private_ips?: boolean;
}

export interface AgentRun {
  id: string;
  tenant_id: string;
  workspace_id: string;
  goal: string;
  status: RunStatus;
  model_used?: string | null;
  iterations: number;
  tokens_in: number;
  tokens_out: number;
  cost_usd: number;
  error_message?: string | null;
  trace_json: string;
  started_at?: string | null;
  completed_at?: string | null;
  created_at: string;
}

export interface AgentStep {
  id: string;
  run_id: string;
  step_index: number;
  phase: StepPhase;
  tool_name?: string | null;
  input_json: string;
  output_json: string;
  tokens_used: number;
  duration_ms: number;
  created_at: string;
}

export interface SecurityScan {
  id: string;
  tenant_id: string;
  workspace_id: string;
  target: string;
  scan_type: ScanType;
  status: string;
  findings_count: number;
  critical_count: number;
  high_count: number;
  started_at?: string | null;
  completed_at?: string | null;
  created_at: string;
}

export interface SecurityFinding {
  id: string;
  scan_id: string;
  tenant_id: string;
  workspace_id: string;
  fingerprint: string;
  category: string;
  severity: Severity;
  title: string;
  description: string;
  evidence: string;
  remediation: string;
  status: FindingStatus;
  created_at: string;
}

export interface Job {
  id: string;
  tenant_id: string;
  workspace_id: string;
  job_type: JobType;
  payload_json: string;
  status: JobStatus;
  attempts: number;
  max_attempts: number;
  lease_expires_at?: string | null;
  run_after_at: string;
  last_error?: string | null;
  created_at: string;
  updated_at: string;
}

export interface UsageEvent {
  id: string;
  tenant_id: string;
  workspace_id: string;
  metric_type: MetricType;
  quantity: number;
  cost_usd: number;
  idempotency_key: string;
  recorded_at: string;
}

export interface WebhookEndpoint {
  id: string;
  tenant_id: string;
  url: string;
  secret_hash: string;
  events_json: string;
  is_active: boolean;
  created_at: string;
}

export interface AuditLog {
  id: string;
  tenant_id: string;
  actor_user_id?: string | null;
  action: string;
  resource_type: string;
  resource_id?: string | null;
  metadata_json: string;
  ip_hash?: string | null;
  created_at: string;
}

/** Consistent API envelope (spec #194). */
export interface ApiEnvelope<T = unknown> {
  ok: boolean;
  data?: T;
  error?: {
    code: string;
    message: string;
    details?: unknown;
  };
  request_id: string;
}

export interface PagedResult<T> {
  items: T[];
  next_cursor?: string | null;
  has_more: boolean;
}
