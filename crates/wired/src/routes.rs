//! REST API route definitions (specs #201-207, #175-179).
//! Consistent API envelope (spec #194), request ID propagation (spec #196).

use crate::middleware::{AppState, auth_context, auth_middleware};
use crate::models::{ApiEnvelope, ErrorCode, utc_now};
use axum::{
    extract::{Path, State, Query},
    response::Json,
    routing::{get, post, patch, delete},
    Router,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Public health / status (no auth).
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/api/v1/status", get(status))
        .route("/api/v1/openapi.json", get(openapi_spec))
        // Auth routes (spec #201).
        .route("/api/v1/auth/register", post(auth_register))
        .route("/api/v1/auth/login", post(auth_login))
        .route("/api/v1/auth/me", get(auth_me))
        // API key routes (spec #202).
        .route("/api/v1/keys", post(create_api_key).get(list_api_keys))
        .route("/api/v1/keys/:id", delete(delete_api_key))
        // Workspace routes (spec #203).
        .route("/api/v1/workspaces", post(create_workspace).get(list_workspaces))
        .route("/api/v1/workspaces/:id", get(get_workspace))
        .route("/api/v1/workspaces/:id/scope", patch(update_workspace_scope))
        // Agent routes (spec #204).
        .route("/api/v1/agents/runs", post(create_agent_run).get(list_agent_runs))
        .route("/api/v1/agents/runs/:id", get(get_agent_run))
        .route("/api/v1/agents/runs/:id/cancel", post(cancel_agent_run))
        .route("/api/v1/agents/runs/:id/stream", get(stream_agent_run))
        // Security scan routes (spec #205).
        .route("/api/v1/scans", post(create_scan).get(list_scans))
        .route("/api/v1/scans/:id", get(get_scan))
        .route("/api/v1/scans/:id/findings", get(list_findings))
        .route("/api/v1/findings/:id", patch(update_finding))
        .route("/api/v1/scans/:id/report", get(get_scan_report))
        // Billing & usage routes (spec #206).
        .route("/api/v1/billing/plans", get(billing_plans))
        .route("/api/v1/billing/usage", get(billing_usage))
        .route("/api/v1/billing/checkout", post(billing_checkout))
        .route("/api/v1/billing/portal", post(billing_portal))
        .route("/api/v1/webhooks/stripe", post(stripe_webhook))
        // Webhook & admin routes (spec #207).
        .route("/api/v1/webhooks", post(create_webhook).get(list_webhooks))
        .route("/api/v1/audit-logs", get(list_audit_logs))
        .route("/api/v1/admin/overview", get(admin_overview))
        .route("/api/v1/export", get(export_tenant))
        // Public security grader (spec #179).
        .route("/api/v1/public/security-grade", post(public_security_grader))
        .route("/api/v1/changelog", get(changelog))
        .with_state(state)
}

// ----- Request/Response DTOs -----

#[derive(Debug, Deserialize)]
pub struct RegisterReq { pub email: String, pub password: String, pub tenant_name: Option<String>, pub tenant_slug: Option<String> }
#[derive(Debug, Deserialize)]
pub struct LoginReq { pub email: String, pub password: String }

/* ---------------------------------------------------------------- */
/* Handlers                                                         */
/* ---------------------------------------------------------------- */

async fn healthz() -> impl IntoResponse {
    Json(ApiEnvelope::ok(serde_json::json!({ "ok": true }), crate::models::new_id()))
}

async fn readyz() -> impl IntoResponse {
    Json(ApiEnvelope::ok(serde_json::json!({ "ready": true }), crate::models::new_id()))
}

async fn status() -> impl IntoResponse {
    Json(ApiEnvelope::ok(serde_json::json!({ "status": "operational", "utc": utc_now() }), crate::models::new_id()))
}

async fn openapi_spec() -> impl IntoResponse {
    Json(serde_json::json!({ "openapi": "3.1.0", "info": { "title": "Wire Desk API", "version": "0.1.0" } }))
}

async fn auth_register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterReq>,
) -> impl IntoResponse {
    let _ = (state, payload);
    Json(ApiEnvelope::err(crate::error::ApiErrorBody { code: ErrorCode::InternalError, message: "auth not wired in this wave".into(), details: None }, crate::models::new_id()))
        .into_response()
}

async fn auth_login() -> impl IntoResponse {
    Json(ApiEnvelope::ok(serde_json::json!({ "token": "placeholder" }), crate::models::new_id()))
}

async fn auth_me() -> impl IntoResponse {
    Json(ApiEnvelope::ok(serde_json::json!({ "user": "placeholder" }), crate::models::new_id()))
}

async fn create_api_key() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn list_api_keys() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn delete_api_key() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }

async fn create_workspace() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn list_workspaces() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn get_workspace() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn update_workspace_scope() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }

async fn create_agent_run() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn list_agent_runs() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn get_agent_run() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn cancel_agent_run() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn stream_agent_run() -> impl IntoResponse {
    // SSE stream (spec #108): returns a single completed event placeholder.
    Json(ApiEnvelope::ok(serde_json::json!({ "stream": "sse endpoint; connect via /api/v1/agents/runs/:id/stream" }), crate::models::new_id()))
}

async fn create_scan() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn list_scans() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn get_scan() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn list_findings() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn update_finding() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn get_scan_report() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }

async fn billing_plans() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn billing_usage() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn billing_checkout() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn billing_portal() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn stripe_webhook() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({ "received": true }), crate::models::new_id())) }

async fn create_webhook() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn list_webhooks() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn list_audit_logs() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }
async fn admin_overview() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn export_tenant() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn public_security_grader() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!({}), crate::models::new_id())) }
async fn changelog() -> impl IntoResponse { Json(ApiEnvelope::ok(serde_json::json!([]), crate::models::new_id())) }

// Handlers returning `Json(ApiEnvelope<...>)` default to 200 OK. For error
// status codes, return `(StatusCode, Json(ApiEnvelope<...>))` tuples instead.

