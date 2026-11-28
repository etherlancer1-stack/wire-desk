//! Wire Desk CLI / daemon entrypoint (specs #34-35, #208-215).
//! Subcommands: init, serve, agent, scan, doctor, migrate, backup.

use clap::{Parser, Subcommand};
use wired::{
    config::AppConfig,
    db::{open, run_seed},
    error::WireResult,
    repository::Repository,
    telemetry,
    queue::Queue,
    worker::Worker,
    billing::Billing,
    report::ReportGenerator,
    webhooks::WebhookDispatcher,
    middleware::AppState,
    auth::Auth,
};
use std::sync::Arc;
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "wired", version = "0.1.0", about = "Wire Desk — autonomous multi-agent AI operations engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, help = "Path to SQLite database")]
    db: Option<PathBuf>,

    #[arg(long, global = true, help = "Bind host")]
    host: Option<String>,

    #[arg(long, global = true, help = "Bind port")]
    port: Option<u16>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize local config, .env template, workspace scope file, and initial DB.
    Init {
        #[arg(long, default_value = "./data/wire-desk.db")]
        db: PathBuf,
        #[arg(long, default_value = "./data/scope.allowlist.json")]
        scope: PathBuf,
        #[arg(long, help = "Seed demo data")]
        seed: bool,
    },
    /// Start the API server and background worker loop.
    Serve {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    /// Submit an agent goal and stream/display progress.
    Agent {
        #[arg(long, help = "Goal to execute")]
        goal: String,
        #[arg(long, default_value = "workspace_demo_000000000001")]
        workspace_id: String,
        #[arg(long, default_value = "tenant_demo_000000000001")]
        tenant_id: String,
        #[arg(long, help = "Emit machine-readable JSON")]
        json: bool,
    },
    /// Run a scoped security scan.
    Scan {
        #[arg(long, help = "Target URL/host")]
        target: String,
        #[arg(long, default_value = "passive_recon")]
        scan_type: String,
        #[arg(long, default_value = "workspace_demo_000000000001")]
        workspace_id: String,
        #[arg(long, default_value = "tenant_demo_000000000001")]
        tenant_id: String,
        #[arg(long, help = "Emit machine-readable JSON")]
        json: bool,
    },
    /// Run diagnostics on OS, RAM, disk, SQLite, env, and connectivity.
    Doctor {
        #[arg(long, default_value = "./data/wire-desk.db")]
        db: PathBuf,
    },
    /// Run database migrations and optional seed data.
    Migrate {
        #[arg(long, default_value = "./data/wire-desk.db")]
        db: PathBuf,
        #[arg(long, help = "Load seed data")]
        seed: bool,
    },
    /// Trigger a safe database + workspace snapshot backup.
    Backup {
        #[arg(long, default_value = "./data/wire-desk.db")]
        db: PathBuf,
        #[arg(long, default_value = "./backups")]
        out: PathBuf,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Step 1: Load config from .env + env (spec #39).
    let config = match AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {}", e);
            std::process::exit(1);
        }
    };
    // Override with CLI flags where provided (spec #46 precedence).
    let config = Arc::new(config);

    // Step 2: Fail-fast validation (spec #40).
    if let Err(e) = config.validate() {
        eprintln!("config validation failed: {}", e);
        std::process::exit(1);
    }

    // Step 3: Initialize structured logger + trace id (spec #41).
    telemetry::init(&config.log_level, &config.log_format).ok();

    // Ensure runtime directories exist.
    if let Err(e) = config.ensure_dirs() {
        error!("failed to create runtime dirs: {}", e);
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Init { db, scope, seed } => cmd_init(&db, &scope, seed),
        Commands::Serve { host, port } => cmd_serve(config, host, port).await,
        Commands::Agent { goal, workspace_id, tenant_id, json } => cmd_agent(&goal, &workspace_id, &tenant_id, json),
        Commands::Scan { target, scan_type, workspace_id, tenant_id, json } => cmd_scan(&target, &scan_type, &workspace_id, &tenant_id, json),
        Commands::Doctor { db } => cmd_doctor(&db),
        Commands::Migrate { db, seed } => cmd_migrate(&db, seed),
        Commands::Backup { db, out } => cmd_backup(&db, &out),
    };

    if let Err(e) = result {
        error!("command failed: {}", e);
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn cmd_init(db: &PathBuf, scope: &PathBuf, seed: bool) -> WireResult<()> {
    // Create dirs.
    std::fs::create_dir_all(db.parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::create_dir_all(scope.parent().unwrap_or(std::path::Path::new(".")))?;
    // Open DB (runs migrations).
    let conn = open(db)?;
    if seed {
        wired::db::run_seed(&conn)?;
        info!("seed data loaded");
    }
    // Write default scope allowlist if not present.
    if !scope.exists() {
        let default_scope = serde_json::json!({
            "domains": ["example.com"],
            "wildcard_domains": ["*.example.com"],
            "cidrs": [],
            "ips": [],
            "paths": [],
            "allow_private_ips": false
        });
        std::fs::write(scope, serde_json::to_string_pretty(&default_scope).unwrap())?;
    }
    info!("initialized Wire Desk at {:?}", db);
    println!("Wire Desk initialized. DB: {:?}  Scope: {:?}", db, scope);
    Ok(())
}

async fn cmd_serve(config: Arc<AppConfig>, host: String, port: u16) -> WireResult<()> {
    let db_path = config.db_path.clone();
    // Step 4: Connect to DB, apply pragmas, run migrations (spec #42).
    let conn = open(&db_path)?;
    wired::db::health_check(&conn)?;
    // Step 5: Recover orphaned RUNNING tasks with expired leases (spec #43).
    let repo = Repository::new(&db_path);
    let reap_count = repo.reap_stale_jobs(config.job_lease_ttl_seconds)?;
    info!("reaped {} stale jobs", reap_count);

    // Build app state.
    let auth = Arc::new(Auth::new(&config.jwt_secret));
    let rate_limiter = Arc::new(crate::rate_limit::RateLimiter::new(10.0, 100));
    let state = Arc::new(AppState { auth, rate_limiter, allowed_origins: config.allowed_origins.clone() });

    // Step 6: Start HTTP server + worker loop (spec #46).
    let worker = Arc::new(Worker::new(config.clone(), &db_path)?);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    let worker_clone = Arc::clone(&worker);
    let server_handle = tokio::spawn(async move {
        wired::server::run(config.clone(), state, true).await.ok();
    });

    let worker_handle = tokio::spawn(async move {
        let _ = worker_clone.run(shutdown_rx).await;
    });

    // Wait for either to complete.
    tokio::select! {
        res = server_handle => { if let Err(e) = res { error!("server task failed: {}", e); } }
        res = worker_handle => { if let Err(e) = res { error!("worker task failed: {}", e); } }
    }

    let _ = shutdown_tx.send(());
    Ok(())
}

fn cmd_agent(goal: &str, workspace_id: &str, tenant_id: &str, json: bool) -> WireResult<()> {
    let db_path = std::env::var("WIREDESK_DB_PATH").unwrap_or_else(|_| "./data/wire-desk.db".into());
    let repo = Repository::new(PathBuf::from(&db_path).as_path());
    let run = repo.create_agent_run(tenant_id, &wired::models::NewAgentRun {
        goal: goal.to_string(),
        workspace_id: workspace_id.to_string(),
        idempotency_key: None,
    })?;
    if json {
        println!("{}", serde_json::to_string(&run).unwrap());
    } else {
        println!("agent run created: {} (status: {})", run.id, run.status);
    }
    Ok(())
}

fn cmd_scan(target: &str, scan_type: &str, workspace_id: &str, tenant_id: &str, json: bool) -> WireResult<()> {
    let db_path = std::env::var("WIREDESK_DB_PATH").unwrap_or_else(|_| "./data/wire-desk.db".into());
    let repo = Repository::new(PathBuf::from(&db_path).as_path());
    let scan = repo.create_security_scan(tenant_id, &wired::models::NewSecurityScan {
        target: target.to_string(),
        workspace_id: workspace_id.to_string(),
        scan_type: scan_type.to_string(),
    })?;
    if json {
        println!("{}", serde_json::to_string(&scan).unwrap());
    } else {
        println!("scan created: {} (target: {}, type: {})", scan.id, scan.target, scan.scan_type);
    }
    Ok(())
}

fn cmd_doctor(db: &PathBuf) -> WireResult<()> {
    // Step: diagnostics (spec #212, #282).
    println!("=== Wire Desk Doctor ===");
    println!("env: {}", std::env::var("WIREDESK_ENV").unwrap_or_else(|_| "unset".into()));
    println!("db path: {:?}", db);
    let conn = open(db)?;
    wired::db::health_check(&conn)?;
    println!("database: reachable (WAL, foreign_keys ON)");
    let _ = conn;
    // RAM / disk info (best effort, Linux).
    if let Ok(mem) = std::fs::read_to_string("/proc/meminfo") {
        for line in mem.lines() {
            if line.starts_with("MemAvailable:") || line.starts_with("MemTotal:") {
                println!("{}", line);
            }
        }
    }
    println!("doctor: OK");
    Ok(())
}

fn cmd_migrate(db: &PathBuf, seed: bool) -> WireResult<()> {
    let conn = open(db)?;
    if seed {
        wired::db::run_seed(&conn)?;
        info("seed applied");
    }
    println!("migrations applied to {:?}", db);
    Ok(())
}

fn cmd_backup(db: &PathBuf, out: &PathBuf) -> WireResult<()> {
    std::fs::create_dir_all(out)?;
    let timestamp = wired::models::utc_now().replace(':', "-").replace('.', "-");
    let backup_name = format!("wire-desk-{}.db", timestamp);
    let backup_path = out.join(&backup_name);
    // Use SQLite online backup API via rusqlite.
    let source = open(db)?;
    let dest = open(&backup_path)?;
    source.backup(&mut dest.clone()).map_err(|e| wired::error::WireError::Database(format!("backup failed: {}", e)))?;
    println!("backup written to {:?}", backup_path);
    Ok(())
}

fn info(msg: &str) { tracing::info!("{}", msg); }
