//! Wire Desk — Rust core crate.
//!
//! Autonomous multi-agent AI operations, defensive security auditing, and task
//! orchestration engine. Low-footprint, multi-tenant, SQLite-backed.

pub mod auth;
pub mod agent;
pub mod billing;
pub mod config;
pub mod db;
pub mod error;
pub mod llm;
pub mod middleware;
pub mod models;
pub mod prompt;
pub mod queue;
pub mod rate_limit;
pub mod report;
pub mod repository;
pub mod router;
pub mod security;
pub mod server;
pub mod routes;
pub mod telemetry;
pub mod tools;
pub mod webhooks;
pub mod worker;

pub use config::AppConfig;
pub use error::{WireError, WireResult};
pub use models::*;
pub use report::*;
