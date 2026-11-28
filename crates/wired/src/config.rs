//! Application configuration.
//!
//! Precedence (spec #46): CLI flags > Environment variables > .env file >
//! JSON/TOML config > Safe defaults. Secrets are read strictly from environment
//! variables (spec #47) and never logged.

use crate::error::WireResult;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

/// Read an environment variable with a fallback default.
fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Read an optional environment variable.
fn env_opt(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// Parse a boolean env var ("1", "true", "yes", "on" => true).
fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(
            v.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

/// Parse a usize env var; on failure return the default (logged by caller).
fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_list(key: &str) -> Vec<String> {
    env::var(key)
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub env: String,
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub scope_file: PathBuf,
    pub config_file: PathBuf,
    pub log_level: String,
    pub log_format: String,
    pub request_id_header: String,
    pub db_journal_mode: String,
    pub db_synchronous: String,
    pub db_busy_timeout_ms: u32,
    pub db_foreign_keys: bool,
    pub db_temp_store: String,
    pub jwt_secret: String,
    pub webhook_secret: String,
    pub allowed_origins: Vec<String>,
    pub public_grader_enabled: bool,
    pub public_grader_rate_limit_rps: f64,
    pub openrouter_api_key: Option<String>,
    pub openrouter_base_url: String,
    pub nvidia_nim_api_key: Option<String>,
    pub nvidia_nim_base_url: String,
    pub gemini_api_key: Option<String>,
    pub gemini_base_url: String,
    pub deepseek_api_key: Option<String>,
    pub deepseek_base_url: String,
    pub model_fast: String,
    pub model_strong: String,
    pub offline_mock: bool,
    pub llm_connect_timeout_ms: u64,
    pub llm_read_timeout_ms: u64,
    pub llm_max_retries: u32,
    pub llm_concurrency: usize,
    pub agent_max_iterations: usize,
    pub agent_tool_output_cap_chars: usize,
    pub job_max_pending_per_tenant: usize,
    pub job_max_pending_global: usize,
    pub job_lease_ttl_seconds: u64,
    pub job_stale_reap_interval_seconds: u64,
    pub worker_concurrency: usize,
    pub sse_buffer_max_events: usize,
    pub cache_max_capacity: usize,
    pub cache_ttl_seconds: u64,
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub stripe_success_url: String,
    pub stripe_cancel_url: String,
    pub billing_enforcement: bool,
    pub e2b_api_key: Option<String>,
    pub e2b_template_id: Option<String>,
    pub referral_code: Option<String>,
    pub onboarding_enabled: bool,
    pub feature_experimental_agents: bool,
    pub feature_security_scanners: bool,
    pub feature_public_routes: bool,
    pub feature_billing_enforcement: bool,
    pub authorized_use_acknowledged: bool,
    pub scope_strict_mode: bool,
}

impl AppConfig {
    /// Load configuration from `.env` (if present, without overriding explicit
    /// env vars) and environment variables. Never overrides an already-set env
    /// var with a `.env` value (dotenvy default behavior).
    pub fn load() -> WireResult<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            env: env_or("WIREDESK_ENV", "development"),
            host: env_or("WIREDESK_HOST", "0.0.0.0"),
            port: env_or("WIREDESK_PORT", "8080")
                .parse()
                .unwrap_or(8080),
            data_dir: PathBuf::from(env_or("WIREDESK_DATA_DIR", "./data")),
            db_path: PathBuf::from(env_or("WIREDESK_DB_PATH", "./data/wire-desk.db")),
            log_dir: PathBuf::from(env_or("WIREDESK_LOG_DIR", "./logs")),
            backup_dir: PathBuf::from(env_or("WIREDESK_BACKUP_DIR", "./backups")),
            tmp_dir: PathBuf::from(env_or("WIREDESK_TMP_DIR", "./tmp")),
            scope_file: PathBuf::from(env_or(
                "WIREDESK_SCOPE_FILE",
                "./data/scope.allowlist.json",
            )),
            config_file: PathBuf::from(env_or(
                "WIREDESK_CONFIG_FILE",
                "./data/config.json",
            )),
            log_level: env_or("WIREDESK_LOG_LEVEL", "info"),
            log_format: env_or("WIREDESK_LOG_FORMAT", "json"),
            request_id_header: env_or("WIREDESK_REQUEST_ID_HEADER", "X-Request-Id"),
            db_journal_mode: env_or("WIREDESK_DB_JOURNAL_MODE", "WAL"),
            db_synchronous: env_or("WIREDESK_DB_SYNCHRONOUS", "NORMAL"),
            db_busy_timeout_ms: env_u32("WIREDESK_DB_BUSY_TIMEOUT_MS", 5000),
            db_foreign_keys: env_bool("WIREDESK_DB_FOREIGN_KEYS", true),
            db_temp_store: env_or("WIREDESK_DB_TEMP_STORE", "MEMORY"),
            jwt_secret: env_or("WIREDESK_JWT_SECRET", "change-me-in-development"),
            webhook_secret: env_or("WIREDESK_WEBHOOK_SECRET", "change-me-in-development"),
            allowed_origins: env_list("WIREDESK_ALLOWED_ORIGINS"),
            public_grader_enabled: env_bool("WIREDESK_PUBLIC_GRADER_ENABLED", true),
            public_grader_rate_limit_rps: env_f64(
                "WIREDESK_PUBLIC_GRADER_RATE_LIMIT_RPS",
                2.0,
            ),
            openrouter_api_key: env_opt("WIREDESK_OPENROUTER_API_KEY"),
            openrouter_base_url: env_or(
                "WIREDESK_OPENROUTER_BASE_URL",
                "https://openrouter.ai/api/v1",
            ),
            nvidia_nim_api_key: env_opt("WIREDESK_NVIDIA_NIM_API_KEY"),
            nvidia_nim_base_url: env_or(
                "WIREDESK_NVIDIA_NIM_BASE_URL",
                "https://integrate.api.nvidia.com/v1",
            ),
            gemini_api_key: env_opt("WIREDESK_GEMINI_API_KEY"),
            gemini_base_url: env_or(
                "WIREDESK_GEMINI_BASE_URL",
                "https://generativelanguage.googleapis.com/v1beta",
            ),
            deepseek_api_key: env_opt("WIREDESK_DEEPSEEK_API_KEY"),
            deepseek_base_url: env_or(
                "WIREDESK_DEEPSEEK_BASE_URL",
                "https://api.deepseek.com/v1",
            ),
            model_fast: env_or("WIREDESK_MODEL_FAST", "mock://offline"),
            model_strong: env_or("WIREDESK_MODEL_STRONG", "openrouter/auto"),
            offline_mock: env_bool("WIREDESK_OFFLINE_MOCK", true),
            llm_connect_timeout_ms: env_u64("WIREDESK_LLM_CONNECT_TIMEOUT_MS", 5000),
            llm_read_timeout_ms: env_u64("WIREDESK_LLM_READ_TIMEOUT_MS", 30000),
            llm_max_retries: env_u32("WIREDESK_LLM_MAX_RETRIES", 3),
            llm_concurrency: env_usize("WIREDESK_LLM_CONCURRENCY", 4),
            agent_max_iterations: env_usize("WIREDESK_AGENT_MAX_ITERATIONS", 10),
            agent_tool_output_cap_chars: env_usize(
                "WIREDESK_AGENT_TOOL_OUTPUT_CAP_CHARS",
                8000,
            ),
            job_max_pending_per_tenant: env_usize(
                "WIREDESK_JOB_MAX_PENDING_PER_TENANT",
                100,
            ),
            job_max_pending_global: env_usize(
                "WIREDESK_JOB_MAX_PENDING_GLOBAL",
                1000,
            ),
            job_lease_ttl_seconds: env_u64("WIREDESK_JOB_LEASE_TTL_SECONDS", 300),
            job_stale_reap_interval_seconds: env_u64(
                "WIREDESK_JOB_STALE_REAP_INTERVAL_SECONDS",
                60,
            ),
            worker_concurrency: env_usize("WIREDESK_WORKER_CONCURRENCY", 4),
            sse_buffer_max_events: env_usize("WIREDESK_SSE_BUFFER_MAX_EVENTS", 500),
            cache_max_capacity: env_usize("WIREDESK_CACHE_MAX_CAPACITY", 1024),
            cache_ttl_seconds: env_u64("WIREDESK_CACHE_TTL_SECONDS", 300),
            stripe_secret_key: env_opt("STRIPE_SECRET_KEY"),
            stripe_publishable_key: env_opt("STRIPE_PUBLISHABLE_KEY"),
            stripe_webhook_secret: env_opt("STRIPE_WEBHOOK_SECRET"),
            stripe_success_url: env_or(
                "WIREDESK_STRIPE_SUCCESS_URL",
                "http://localhost:8080/billing/success",
            ),
            stripe_cancel_url: env_or(
                "WIREDESK_STRIPE_CANCEL_URL",
                "http://localhost:8080/billing/cancel",
            ),
            billing_enforcement: env_bool("WIREDESK_BILLING_ENFORCEMENT", true),
            e2b_api_key: env_opt("E2B_API_KEY"),
            e2b_template_id: env_opt("E2B_TEMPLATE_ID"),
            referral_code: env_opt("WIREDESK_REFERRAL_CODE"),
            onboarding_enabled: env_bool("WIREDESK_ONBOARDING_ENABLED", true),
            feature_experimental_agents: env_bool(
                "WIREDESK_FEATURE_EXPERIMENTAL_AGENTS",
                false,
            ),
            feature_security_scanners: env_bool(
                "WIREDESK_FEATURE_SECURITY_SCANNERS",
                true,
            ),
            feature_public_routes: env_bool("WIREDESK_FEATURE_PUBLIC_ROUTES", true),
            feature_billing_enforcement: env_bool(
                "WIREDESK_FEATURE_BILLING_ENFORCEMENT",
                true,
            ),
            authorized_use_acknowledged: env_bool(
                "WIREDESK_AUTHORIZED_USE_ACKNOWLEDGED",
                false,
            ),
            scope_strict_mode: env_bool("WIREDESK_SCOPE_STRICT_MODE", true),
        })
    }

    /// Fail-fast validation (spec #40, #241). Never silent bad config.
    pub fn validate(&self) -> WireResult<()> {
        if self.env == "production" {
            let weak = self.jwt_secret == "change-me-in-development"
                || self.jwt_secret.len() < 32;
            if weak {
                return Err(WireError::Config(
                    "WIREDESK_JWT_SECRET must be >= 32 chars and not the default in production"
                        .to_string(),
                ));
            }
            let webhook_weak = self.webhook_secret == "change-me-in-development"
                || self.webhook_secret.len() < 32;
            if webhook_weak {
                return Err(WireError::Config(
                    "WIREDESK_WEBHOOK_SECRET must be >= 32 chars and not the default in production"
                        .to_string(),
                ));
            }
            if self.feature_public_routes && !self.authorized_use_acknowledged {
                return Err(WireError::Config(
                    "WIREDESK_AUTHORIZED_USE_ACKNOWLEDGED must be true when public routes are enabled in production"
                        .to_string(),
                ));
            }
        }
        if self.job_max_pending_per_tenant == 0 {
            return Err(WireError::Config(
                "WIREDESK_JOB_MAX_PENDING_PER_TENANT must be > 0".to_string(),
            ));
        }
        if self.agent_max_iterations == 0 || self.agent_max_iterations > 100 {
            return Err(WireError::Config(
                "WIREDESK_AGENT_MAX_ITERATIONS must be between 1 and 100".to_string(),
            ));
        }
        Ok(())
    }

    pub fn is_production(&self) -> bool {
        self.env == "production"
    }

    pub fn is_test(&self) -> bool {
        self.env == "test"
    }

    /// Ensure runtime directories exist (data, logs, backups, tmp).
    pub fn ensure_dirs(&self) -> WireResult<()> {
        for dir in [
            &self.data_dir,
            &self.log_dir,
            &self.backup_dir,
            &self.tmp_dir,
        ] {
            std::fs::create_dir_all(dir).map_err(|e| {
                WireError::Database(format!("failed to create dir {:?}: {}", dir, e))
            })?;
        }
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        // Safe defaults; callers should call load() + validate() instead.
        Self {
            env: "development".into(),
            host: "127.0.0.1".into(),
            port: 8080,
            data_dir: PathBuf::from("./data"),
            db_path: PathBuf::from("./data/wire-desk.db"),
            log_dir: PathBuf::from("./logs"),
            backup_dir: PathBuf::from("./backups"),
            tmp_dir: PathBuf::from("./tmp"),
            scope_file: PathBuf::from("./data/scope.allowlist.json"),
            config_file: PathBuf::from("./data/config.json"),
            log_level: "info".into(),
            log_format: "plain".into(),
            request_id_header: "X-Request-Id".into(),
            db_journal_mode: "WAL".into(),
            db_synchronous: "NORMAL".into(),
            db_busy_timeout_ms: 5000,
            db_foreign_keys: true,
            db_temp_store: "MEMORY".into(),
            jwt_secret: "change-me-in-development".into(),
            webhook_secret: "change-me-in-development".into(),
            allowed_origins: vec!["http://localhost:8080".into()],
            public_grader_enabled: true,
            public_grader_rate_limit_rps: 2.0,
            openrouter_api_key: None,
            openrouter_base_url: "https://openrouter.ai/api/v1".into(),
            nvidia_nim_api_key: None,
            nvidia_nim_base_url: "https://integrate.api.nvidia.com/v1".into(),
            gemini_api_key: None,
            gemini_base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
            deepseek_api_key: None,
            deepseek_base_url: "https://api.deepseek.com/v1".into(),
            model_fast: "mock://offline".into(),
            model_strong: "openrouter/auto".into(),
            offline_mock: true,
            llm_connect_timeout_ms: 5000,
            llm_read_timeout_ms: 30000,
            llm_max_retries: 3,
            llm_concurrency: 4,
            agent_max_iterations: 10,
            agent_tool_output_cap_chars: 8000,
            job_max_pending_per_tenant: 100,
            job_max_pending_global: 1000,
            job_lease_ttl_seconds: 300,
            job_stale_reap_interval_seconds: 60,
            worker_concurrency: 4,
            sse_buffer_max_events: 500,
            cache_max_capacity: 1024,
            cache_ttl_seconds: 300,
            stripe_secret_key: None,
            stripe_publishable_key: None,
            stripe_webhook_secret: None,
            stripe_success_url: "http://localhost:8080/billing/success".into(),
            stripe_cancel_url: "http://localhost:8080/billing/cancel".into(),
            billing_enforcement: true,
            e2b_api_key: None,
            e2b_template_id: None,
            referral_code: None,
            onboarding_enabled: true,
            feature_experimental_agents: false,
            feature_security_scanners: true,
            feature_public_routes: true,
            feature_billing_enforcement: true,
            authorized_use_acknowledged: false,
            scope_strict_mode: true,
        }
    }
}

pub const DEFAULT_JWT_SECRET: &str = "change-me-in-development";
pub const DEFAULT_WEBHOOK_SECRET: &str = "change-me-in-development";
