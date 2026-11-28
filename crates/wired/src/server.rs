//! Axum HTTP server + graceful shutdown (specs #45, #176, #177, #298, #299).

use crate::config::AppConfig;
use crate::middleware::{AppState, auth_middleware, cors_middleware, rate_limit_middleware, request_id_middleware, security_headers_middleware};
use crate::routes::router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing::{error, info};

pub async fn run(config: Arc<AppConfig>, state: Arc<AppState>, _db_ready: bool) -> std::io::Result<()> {
    let app = router(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // spec #197: 1MB body limit
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(axum::middleware::from_fn(cors_middleware))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(axum::middleware::from_fn(rate_limit_middleware));

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()
        .expect("invalid bind address");
    info!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| error!("server error: {}", e))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.ok();
    };
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .ok()
            .and_then(|mut h| { h.recv().await; Some(()) })
            .await;
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutdown signal received, draining...");
}
