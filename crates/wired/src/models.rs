//! Shared domain models used across Rust persistence, queue, agent, and API
//! layers. Types mirror `src-ts/db/schema.ts` and `migrations/0001_initial.sql`
//! exactly (spec #53, #328).

use crate::error::WireResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plan tiers (spec #164). Aligned with `tenants.plan_tier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanTier {
    Free,
    Pro,
    Team,
    Enterprise,
}

impl PlanTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanTier::Free => "free",
            PlanTier::Pro => "pro",
            PlanTier::Team => "team",
            PlanTier::Enterprise => "enterprise",
        }
    }

    pub fn from_str(s: &str) -> WireResult<Self> {
        match s {
            "free" => Ok(PlanTier::Free),
            "pro" => Ok(PlanTier::Pro),
            "team" => Ok(PlanTier::Team),
            "enterprise" => Ok(PlanTier::Enterprise),
            other => Err(WireError::Validation(format!("unknown plan tier: {}", other))),
        }
    }

    /// Monthly token quota per tier (spec #164).
    pub fn monthly_token_quota(&self) -> i64 {
        match self {
            PlanTier::Free => 50_000,
            PlanTier::Pro => 1_500_000,
            PlanTier::Team => 7_500_000,
            PlanTier::Enterprise => 50_000_000,
        }
    }

    /// Monthly scan quota per tier (spec #164).
    pub fn monthly_scan_quota(&self) -> i64 {
        match self {
            PlanTier::Free => 100,
            PlanTier::Pro => 200,
            PlanTier::Team => 1000,
            PlanTier::Enterprise => 10_000,
        }
    }

    /// Max workspaces per tier (spec #164).
    pub fn max_workspaces(&self) -> i64 {
        match self {
            PlanTier::Free => 1,
            PlanTier::Pro => 5,
            PlanTier::Team => 25,
            PlanTier::Enterprise => 1000,
        }
    }
}

/// User roles (spec #163). Aligned with `users.role`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Analyst,
    Viewer,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Analyst => "analyst",
            Role::Viewer => "viewer",
        }
    }
}

/// Agent run status (spec #62).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Running => "running",
            RunStatus::WaitingApproval => "waiting_approval",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

/// Job status (spec #66).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    DeadLetter,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::DeadLetter => "dead_letter",
        }
    }
}

/// Job type (spec #66).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    AgentRun,
    SecurityScan,
    WebhookDelivery,
    ReportExport,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::AgentRun => "agent_run",
            JobType::SecurityScan => "security_scan",
            JobType::WebhookDelivery => "webhook_delivery",
            JobType::ReportExport => "report_export",
        }
    }
}

/// Scan type (spec #64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanType {
    PassiveRecon,
    DnsTlsAudit,
    SastRepo,
    DepAudit,
    SsrfCheck,
}

impl ScanType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanType::PassiveRecon => "passive_recon",
            ScanType::DnsTlsAudit => "dns_tls_audit",
            ScanType::SastRepo => "sast_repo",
            ScanType::DepAudit => "dep_audit",
            ScanType::SsrfCheck => "ssrf_check",
        }
    }
}

/// Finding severity (spec #65).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Info => "info",
        }
    }
}

/// Finding status (spec #65, #155).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    Open,
    Triaged,
    Resolved,
    FalsePositive,
}

impl FindingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingStatus::Open => "open",
            FindingStatus::Triaged => "triaged",
            FindingStatus::Resolved => "resolved",
            FindingStatus::FalsePositive => "false_positive",
        }
    }
}

/// Metric type (spec #67).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    AiTokens,
    AiRuns,
    Scans,
    SandboxSec,
    ApiCalls,
}

impl MetricType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::AiTokens => "ai_tokens",
            MetricType::AiRuns => "ai_runs",
            MetricType::Scans => "scans",
            MetricType::SandboxSec => "sandbox_sec",
            MetricType::ApiCalls => "api_calls",
        }
    }
}

/// Phase of an agent step (spec #63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepPhase {
    Plan,
    ToolCall,
    ToolResult,
    Reflection,
    Final,
}

impl StepPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepPhase::Plan => "plan",
            StepPhase::ToolCall => "tool_call",
            StepPhase::ToolResult => "tool_result",
            StepPhase::Reflection => "reflection",
            StepPhase::Final => "final",
        }
    }
}

/* ---------------------------------------------------------------- */
/* Row structs (mirror DB tables)                                   */
/* ---------------------------------------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub plan_tier: String,
    pub stripe_customer_id: Option<String>,
    pub stripe_subscription_id: Option<String>,
    pub monthly_token_quota: i64,
    pub monthly_scan_quota: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub tenant_id: String,
    pub email: String,
    pub role: String,
    pub password_hash: String,
    pub api_key_prefix: String,
    pub api_key_hash: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: String,
    pub scope_allowlist_json: String,
    pub rate_limit_rps: f64,
    pub config_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub goal: String,
    pub status: String,
    pub model_used: Option<String>,
    pub iterations: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_usd: f64,
    pub error_message: Option<String>,
    pub trace_json: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub id: String,
    pub run_id: String,
    pub step_index: i64,
    pub phase: String,
    pub tool_name: Option<String>,
    pub input_json: String,
    pub output_json: String,
    pub tokens_used: i64,
    pub duration_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScan {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub target: String,
    pub scan_type: String,
    pub status: String,
    pub findings_count: i64,
    pub critical_count: i64,
    pub high_count: i64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub id: String,
    pub scan_id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub fingerprint: String,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub evidence: String,
    pub remediation: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub job_type: String,
    pub payload_json: String,
    pub status: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub lease_expires_at: Option<String>,
    pub run_after_at: String,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub metric_type: String,
    pub quantity: i64,
    pub cost_usd: f64,
    pub idempotency_key: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub tenant_id: String,
    pub url: String,
    pub secret_hash: String,
    pub events_json: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    pub tenant_id: String,
    pub actor_user_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub metadata_json: String,
    pub ip_hash: Option<String>,
    pub created_at: String,
}

/* ---------------------------------------------------------------- */
/* Insert / request payloads                                        */
/* ---------------------------------------------------------------- */

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewTenant {
    pub name: String,
    pub slug: String,
    pub plan_tier: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewWorkspace {
    pub name: String,
    pub description: Option<String>,
    pub scope_allowlist: Option<ScopeAllowlist>,
    pub rate_limit_rps: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewAgentRun {
    pub goal: String,
    pub workspace_id: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewSecurityScan {
    pub target: String,
    pub workspace_id: String,
    pub scan_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewJob {
    pub tenant_id: String,
    pub workspace_id: String,
    pub job_type: String,
    pub payload_json: Option<String>,
    pub max_attempts: Option<i64>,
    pub run_after_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewAuditLog {
    pub tenant_id: String,
    pub actor_user_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub metadata_json: Option<String>,
    pub ip_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScopeAllowlist {
    pub domains: Vec<String>,
    pub wildcard_domains: Vec<String>,
    pub cidrs: Vec<String>,
    pub ips: Vec<String>,
    pub paths: Vec<String>,
    #[serde(default)]
    pub allow_private_ips: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiEnvelope<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<crate::error::ApiErrorBody>,
    pub request_id: String,
}

impl<T> ApiEnvelope<T> {
    pub fn ok(data: T, request_id: String) -> Self {
        Self { ok: true, data: Some(data), error: None, request_id }
    }
    pub fn err(error: crate::error::ApiErrorBody, request_id: String) -> Self {
        Self { ok: false, data: None, error: Some(error), request_id }
    }
}

pub type HashMapStr = HashMap<String, String>;

/// UTC timestamp helper (spec #81).
pub fn utc_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Generate a new UUID v4 string.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
