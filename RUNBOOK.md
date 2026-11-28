# Wire Desk — Runbook

## Startup

```bash
# Build
make build-rust

# Init DB, config, scope
make init

# Seed demo data
make seed

# Start server + worker
make serve
```

Startup lifecycle (specs #39-46):
1. Load `.env` + parse CLI flags
2. Fail-fast config validation
3. Structured logger + `trace_id`
4. Connect SQLite, apply WAL/NORMAL/foreign_keys/busy_timeout, run migrations
5. Recover orphaned `RUNNING` tasks → `PENDING`
6. Warm caches, register providers, start HTTP server + worker loop

## Health checks

```bash
curl http://localhost:8080/healthz   # liveness
curl http://localhost:8080/readyz    # readiness (DB + config)
curl http://localhost:8080/api/v1/status
```

## Doctor

```bash
./crates/wired/target/release/wired doctor --db data/wire-desk.db
```

Checks env vars, DB connectivity & WAL mode, disk, memory, and network reachability (specs #212, #282).

## Backup / restore

```bash
# Backup
./scripts/backup.sh
# -> backups/wired-backup-20261128-120000.tar.gz

# Restore
./scripts/restore.sh backups/wired-backup-20261128-120000.tar.gz
```

Backups use SQLite online backup + tar/gzip, with retention pruning
(`BACKUP_RETENTION_DAYS`, default 7). Restore validates integrity before
replacing the live DB (spec #226).

## Daemon / process supervision

```bash
# Foreground watchdog with crash backoff
./scripts/run_daemon.sh watchdog

# Via tmux
./scripts/run_daemon.sh tmux

# Docker
docker compose up -d
```

Process supervision: tmux, shell watchdog loops with exponential backoff,
Docker restart policies, or cloud process managers (spec #15).

## Graceful shutdown

`SIGINT` / `SIGTERM` → stop accepting new work → drain active tasks within
deadline → flush buffers → close DB cleanly (spec #45).

## Key rotation

1. Create a new API key (`POST /api/v1/keys`) — raw shown once
2. Update clients to the new key
3. Revoke the old key (`DELETE /api/v1/keys/:id`) — zero downtime (#275)

JWT secret rotation: regenerate `WIREDESK_JWT_SECRET` and restart; existing
sessions become invalid (re-login required).

## Incident response

1. `curl /readyz` — confirm DB reachable
2. Check `logs/` (JSON) for errors; secrets are redacted (#146)
3. Reap stale jobs: the worker reaps expired leases automatically (#265)
4. Dead-letter queue: inspect `jobs` where `status='dead_letter'`
5. Restore from backup if DB corrupted (see above)

## Scaling

- Single SQLite file; add read replicas via file copy for read-heavy workloads
- `WIREDESK_WORKER_CONCURRENCY` bounds concurrent jobs (default 4)
- `WIREDESK_LLM_CONCURRENCY` bounds outbound LLM requests (default 4)
- `WIREDESK_JOB_MAX_PENDING_GLOBAL` backpressure guard (default 1000)

## Tier limits

| Plan | AI runs/mo | Scans/mo | Workspaces |
|------|-----------|----------|------------|
| free | 50 | 100 | 1 |
| pro | 1,500 | 200 | 5 |
| team | 7,500 | 1,000 | 25 |
| enterprise | custom | custom | custom |
