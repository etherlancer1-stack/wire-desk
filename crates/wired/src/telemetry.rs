//! Structured logging and request/trace ID propagation (spec #41, #196).
//! Uses `tracing` with a `RequestId` layer attached via Axum middleware.

use once_cell::sync::Lazy;
use tracing::subscriber::set_global_default;
use tracing::{field::display, info, warn};
use tracing_subscriber::{
    fmt::{format::JsonFields, time::UtcTime, FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

static TRACE_ID_FIELD: Lazy<std::sync::atomic::AtomicU64> =
    Lazy::new(|| std::sync::atomic::AtomicU64::new(1));

/// Generate a monotonically increasing trace id (unique within a process).
pub fn next_trace_id() -> String {
    let n = TRACE_ID_FIELD.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("tr-{:016x}", n)
}

/// Initialize the global tracing subscriber based on config.
pub fn init(log_level: &str, log_format: &str) -> WireResult<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_timer(UtcTime::rfc_3339())
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true);

    let layer: Box<dyn Layer<tracing_subscriber::registry::Registry> + Send + Sync> =
        match log_format {
            "json" => Box::new(fmt_layer.json().with_ansi(false)),
            _ => Box::new(fmt_layer.with_ansi(true)),
        };

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init()
        .ok();

    info!("wire-desk telemetry initialized");
    Ok(())
}

use crate::error::WireResult;
