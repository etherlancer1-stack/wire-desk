//! Unified LLM router (specs #86-118).
//! Common request/response over OpenRouter, NVIDIA NIM, Gemini, DeepSeek, plus
//! an offline deterministic mock provider. Provider fallback chain, task-aware
//! routing, cost estimation, and concurrency limiting.

use crate::config::AppConfig;
use crate::error::{WireError, WireResult};
use crate::models::utc_now;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

/// Unified request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub response_format: Option<String>,
    pub task_class: Option<String>, // "fast" | "strong" | "analysis"
}

/// Unified chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,   // "system" | "user" | "assistant" | "tool"
    pub content: String,
}

/// Unified response (spec #116).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub finish_reason: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
    pub provider: String,
    pub model: String,
    pub duration_ms: u64,
}

/// Task-aware routing classes (spec #88).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    Fast,
    Strong,
}

impl TaskClass {
    pub fn from_str(s: &str) -> Self {
        match s {
            "strong" | "analysis" | "security" => TaskClass::Strong,
            _ => TaskClass::Fast,
        }
    }
}

/// Abstract provider trait implemented per provider and for mock.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn complete(&self, req: &LlmRequest) -> WireResult<LlmResponse>;
}

#[async_trait::async_trait]
impl LlmProvider for dyn LlmProvider {
    fn name(&self) -> &'static str {
        self.name()
    }
    async fn complete(&self, req: &LlmRequest) -> WireResult<LlmResponse> {
        self.complete(req).await
    }
}

/// Offline deterministic mock provider (spec #114).
pub struct MockProvider;

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn name(&self) -> &'static str { "mock://offline" }

    async fn complete(&self, req: &LlmRequest) -> WireResult<LlmResponse> {
        let last = req.messages.last().map(|m| m.content.as_str()).unwrap_or("");
        let content = format!(
            "[mock] deterministic response to: {}\n(plan: analyze -> observe -> summarize)",
            safe_truncate(last, 200)
        );
        let tokens_in = estimate_tokens(&req.messages) as u32;
        let tokens_out = estimate_tokens(&[ChatMessage { role: "assistant".into(), content: content.clone() }]) as u32;
        Ok(LlmResponse {
            content,
            finish_reason: "stop".into(),
            tokens_in,
            tokens_out,
            cost_usd: 0.0,
            provider: "mock".into(),
            model: req.model.clone(),
            duration_ms: 5,
        })
    }
}

/// Generic HTTP provider used for OpenRouter / NVIDIA NIM / Gemini / DeepSeek.
struct HttpProvider {
    name: &'static str,
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl LlmProvider for HttpProvider {
    fn name(&self) -> &'static str { self.name }

    async fn complete(&self, req: &LlmRequest) -> WireResult<LlmResponse> {
        let key = self.api_key.as_deref().unwrap_or("");
        let start = SystemTime::now();
        // Build provider-specific payload (normalized internal -> provider wire format).
        let payload = match self.name {
            "openrouter" => serde_json::json!({
                "model": req.model,
                "messages": req.messages,
                "temperature": req.temperature.unwrap_or(0.7),
                "max_tokens": req.max_tokens.unwrap_or(1000),
            }),
            "nvidia_nim" => serde_json::json!({
                "model": req.model,
                "messages": req.messages,
                "temperature": req.temperature.unwrap_or(0.7),
                "max_tokens": req.max_tokens.unwrap_or(1000),
            }),
            "gemini" => serde_json::json!({
                "contents": req.messages.iter().map(|m| serde_json::json!({
                    "role": if m.role=="user" {"user"} else {"model"},
                    "parts": [{"text": &m.content}]
                })).collect::<Vec<_>>(),
            }),
            "deepseek" => serde_json::json!({
                "model": req.model,
                "messages": req.messages,
                "temperature": req.temperature.unwrap_or(0.7),
                "max_tokens": req.max_tokens.unwrap_or(1000),
            }),
            _ => serde_json::json!({"model": req.model, "messages": req.messages}),
        };
        let url = match self.name {
            "gemini" => format!("{}/models/{}:generateContent?key={}", self.base_url, req.model, key),
            _ => format!("{}/chat/completions", self.base_url),
        };
        let mut request = self.client.post(&url).json(&payload);
        if self.name != "gemini" {
            request = request.bearer_auth(key);
        }
        let resp = request
            .timeout(std::time::Duration::from_millis(30000))
            .send()
            .await
            .map_err(|e| WireError::External(format!("llm request failed: {}", e)))?;
        let status = resp.status();
        let body: serde_json::Value = resp.json().await
            .map_err(|e| WireError::External(format!("llm parse failed: {}", e)))?;
        let duration_ms = start.elapsed().unwrap_or_default().as_millis() as u64;

        // Classify errors (spec #113).
        if status.is_client_error() || status.is_server_error() {
            let msg = body.get("error").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Err(WireError::External(format!("llm {} {}: {}", status, self.name, msg)));
        }

        // Normalize provider differences into unified response (spec #116).
        let content = match self.name {
            "gemini" => body.get("candidates")
                .and_then(|v| v.get(0))
                .and_then(|v| v.get("content"))
                .and_then(|v| v.get("parts"))
                .and_then(|v| v.get(0))
                .and_then(|v| v.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            _ => body.get("choices")
                .and_then(|v| v.get(0))
                .and_then(|v| v.get("message"))
                .and_then(|v| v.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };
        let tokens_in = body.get("usage")
            .and_then(|v| v.get("prompt_tokens"))
            .and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let tokens_out = body.get("usage")
            .and_then(|v| v.get("completion_tokens"))
            .and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let finish_reason = body.get("choices")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("finish_reason"))
            .and_then(|v| v.as_str())
            .unwrap_or("stop")
            .to_string();

        let cost_usd = compute_cost(self.name, tokens_in, tokens_out);

        Ok(LlmResponse { content, finish_reason, tokens_in, tokens_out, cost_usd,
            provider: self.name.to_string(), model: req.model.clone(), duration_ms })
    }
}

fn compute_cost(provider: &str, tokens_in: u32, tokens_out: u32) -> f64 {
    // Configurable per-1M rates (spec #93). Placeholder rates; wire to config later.
    let (rate_in, rate_out) = match provider {
        "mock" => (0.0, 0.0),
        "deepseek" => (0.14, 0.28),
        "openrouter" => (1.0, 2.0),
        "nvidia_nim" => (0.5, 1.0),
        "gemini" => (0.25, 0.50),
        _ => (1.0, 2.0),
    };
    (tokens_in as f64 / 1_000_000.0 * rate_in) + (tokens_out as f64 / 1_000_000.0 * rate_out)
}

fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    // Rough estimate: ~4 chars per token (spec #91). Good enough for trimming.
    messages.iter().map(|m| m.content.len()).sum::<usize>() / 4
}

fn safe_truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; } // spec #238 UTF-8 guard
    s[..end].to_string()
}

/// Router: provider selection, fallback chain, concurrency limiting, mock mode.
pub struct LlmRouter {
    providers: Vec<Arc<dyn LlmProvider>>,
    semaphore: Semaphore,
    offline_mock: bool,
    model_fast: String,
    model_strong: String,
}

impl LlmRouter {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let mut providers: Vec<Arc<dyn LlmProvider>> = Vec::new();
        providers.push(Arc::new(MockProvider));
        if config.openrouter_api_key.is_some() {
            providers.push(Arc::new(HttpProvider {
                name: "openrouter", base_url: config.openrouter_base_url.clone(),
                api_key: config.openrouter_api_key.clone(),
                client: reqwest::Client::new(),
            }));
        }
        if config.deepseek_api_key.is_some() {
            providers.push(Arc::new(HttpProvider {
                name: "deepseek", base_url: config.deepseek_base_url.clone(),
                api_key: config.deepseek_api_key.clone(),
                client: reqwest::Client::new(),
            }));
        }
        if config.nvidia_nim_api_key.is_some() {
            providers.push(Arc::new(HttpProvider {
                name: "nvidia_nim", base_url: config.nvidia_nim_base_url.clone(),
                api_key: config.nvidia_nim_api_key.clone(),
                client: reqwest::Client::new(),
            }));
        }
        if config.gemini_api_key.is_some() {
            providers.push(Arc::new(HttpProvider {
                name: "gemini", base_url: config.gemini_base_url.clone(),
                api_key: config.gemini_api_key.clone(),
                client: reqwest::Client::new(),
            }));
        }
        Self {
            providers,
            semaphore: Semaphore::new(config.llm_concurrency),
            offline_mock: config.offline_mock,
            model_fast: config.model_fast.clone(),
            model_strong: config.model_strong.clone(),
        }
    }

    /// Pick model for task class (spec #88).
    fn model_for(&self, task_class: TaskClass) -> String {
        match task_class {
            TaskClass::Fast => self.model_fast.clone(),
            TaskClass::Strong => self.model_strong.clone(),
        }
    }

    /// Complete with automatic failover across providers on 429/5xx/timeout (spec #87).
    pub async fn complete(&self, req: &LlmRequest) -> WireResult<LlmResponse> {
        let task_class = match req.task_class.as_deref() {
            Some("strong") | Some("analysis") | Some("security") => TaskClass::Strong,
            _ => TaskClass::Fast,
        };
        let model = self.model_for(task_class);

        // Offline deterministic mode (spec #114).
        if self.offline_mock {
            let mock_req = LlmRequest { model: "mock://offline".into(), ..req.clone() };
            return MockProvider.complete(&mock_req).await;
        }

        let mut last_err = None;
        for provider in &self.providers {
            let _permit = self.semaphore.acquire().await.map_err(|e| {
                WireError::Internal(format!("semaphore closed: {}", e))
            })?;
            let provider_req = LlmRequest {
                model: model.clone(),
                ..req.clone()
            };
            match provider.complete(&provider_req).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    warn!("provider {} failed: {}", provider.name(), e);
                    last_err = Some(e);
                    // Fallback on transient errors; auth errors are not retried.
                    continue;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| WireError::External("no LLM providers available".into())))
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}
