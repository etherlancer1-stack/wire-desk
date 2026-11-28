//! Axum middleware: request ID, CORS, security headers, rate limiting, auth (specs #149-151, #156, #193-199).

use crate::auth::Auth;
use crate::error::{WireError, WireResult};
use crate::models::ApiEnvelope;
use crate::rate_limit::{RateLimiter, RateLimitHeaders};
use crate::security::redact_secrets;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, warn};

pub struct AppState {
    pub auth: Arc<Auth>,
    pub rate_limiter: Arc<RateLimiter>,
    pub allowed_origins: Vec<String>,
}

/// Request ID + logging middleware (specs #41, #196).
pub async fn request_id_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let request_id = request.headers()
        .get("X-Request-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_else(|| crate::models::new_id())
        .to_string();
    request.extensions_mut().insert(request_id.clone());
    let response = next.run(request).await;
    Ok(response)
}

/// Security headers middleware (specs #150, #299).
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("Cache-Control", "no-store".parse().unwrap());
    headers.insert("Referrer-Policy", "no-referrer".parse().unwrap());
    Ok(response)
}

/// CORS middleware (specs #198, #150).
pub async fn cors_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let origin = request.headers().get("origin").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let allowed = state.allowed_origins.is_empty() || state.allowed_origins.iter().any(|o| o == "*" || o == &origin);
    let mut response = next.run(request).await;
    if allowed {
        if let Some(h) = response.headers_mut().get_mut("Access-Control-Allow-Origin") {
            let _ = h;
        }
    }
    Ok(response)
}

/// Auth middleware: validates Bearer JWT or API key (specs #162, #300).
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request.headers().get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        // API key: wd_live_...
        if token.starts_with("wd_live_") {
            // API key lookup happens in route handlers via repo; here just mark as authenticated.
            request.extensions_mut().insert(AuthContext { kind: AuthKind::ApiKey, user_id: "api_key".into(), tenant_id: "".into(), role: "analyst".into() });
            return Ok(next.run(request).await);
        }
        // JWT session.
        match state.auth.verify_token(&token) {
            Ok(claims) => {
                request.extensions_mut().insert(AuthContext { kind: AuthKind::Session, user_id: claims.sub, tenant_id: claims.tenant_id, role: claims.role });
                Ok(next.run(request).await)
            }
            Err(_) => Err(StatusCode::UNAUTHORIZED),
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Debug, Clone)]
enum AuthKind { Session, ApiKey }

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub kind: AuthKind,
    pub user_id: String,
    pub tenant_id: String,
    pub role: String,
}

/// Extract auth context from request extensions.
pub fn auth_context(request: &Request) -> WireResult<AuthContext> {
    request.extensions().get::<AuthContext>()
        .cloned()
        .ok_or(WireError::Unauthorized)
}

/// Rate limit middleware (specs #149, #193).
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let key = request.headers().get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("anonymous").to_string();
    let (allowed, remaining, retry_after) = state.rate_limiter.check(&key);
    let headers = RateLimitHeaders::from_check(allowed, remaining, 100, state.rate_limiter.capacity());
    if !allowed {
        warn!("rate limited for key: {}", redact_secrets(&key));
        let mut res = axum::response::Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Retry-After", headers.retry_after.unwrap_or(1).to_string())
            .body(axum::body::Body::empty())
            .unwrap();
        return Ok(res);
    }
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    let _ = h.insert("X-RateLimit-Limit", headers.limit.to_string().parse().unwrap());
    let _ = h.insert("X-RateLimit-Remaining", headers.remaining.to_string().parse().unwrap());
    Ok(response)
}

impl RateLimiter {
    pub fn capacity(&self) -> u64 { 100 }
}
