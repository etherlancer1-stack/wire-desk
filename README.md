# Wire Desk

Autonomous multi-agent AI operations, defensive security auditing, and task
orchestration engine built with Rust, TypeScript, and SQLite for low-footprint
environments and multi-tenant SaaS scale.

**Primary repository:** https://github.com/etherlancer1/wire-desk
**Reference upstream:** https://github.com/wireapp/wire-desktop

## Architecture

```
+---------------------+      +---------------------+      +---------------------+
|   Web Dashboard     |      |   CLI (wired)       |      |  Public Security    |
|   (web/index.html)  |      |   (init/serve/..)   |      |  Grader             |
+----------+----------+      +----------+----------+      +----------+----------+
           |                           |                           |
           v                           v                           v
+---------------------------------------------------------------+
|                   Axum HTTP Server (Rust)                     |
|  /api/v1/{auth,keys,workspaces,agents,scans,billing,webhooks}  |
+-----------------------------------+---------------------------+
                                    |
              +---------------------+---------------------+
              |                                             |
   +----------v----------+                          +------v------+
   |   Durable Job Queue |                          |   LLM Router  |
   |   (SQLite leasing)  |                          |   (fallback)  |
   +----------+----------+                          +------+--------+
              |                                             |
   +----------v----------+                          +--------v--------+
   |   Agent Engine      |                          |   OpenRouter/   |
   |   (state machine)   |                          |   NVIDIA/Gemini/|
   +----------+----------+                          |   DeepSeek/mock |
              |                                     +--------+--------+
   +----------v----------+                           |         |
   |   Tool Registry     |                    +------v---+ +---v------+
   |   sandbox/file/http |                    | Security | |  Recon   |
   +----------+----------+                    | Scanner  | |  (DoH)   |
              |                              +----+-----+ +-----+----+
   +----------v----------+                             |         |
   |   SQLite (WAL)      |                      +-------v-+ +---v---+
   | tenants/users/...   |                      | SAST    | | DNS/  |
   +---------------------+                      | deps    | | TLS   |
                                                +---------+ +-------+
```

## Quickstart

```bash
# 1. Clone
git clone https://github.com/etherlancer1/wire-desk.git
cd wire-desk

# 2. Install Rust (https://rustup.rs) then build
make build-rust

# 3. Initialize DB + config + scope
make init

# 4. Seed demo data (optional)
make seed

# 5. Start server + worker
make serve

# 6. Visit http://localhost:8080 (dashboard) or use the CLI:
./crates/wired/target/release/wired doctor --db data/wire-desk.db
./crates/wired/target/release/wired agent --goal "Summarize security posture of example.com"
```

## CLI subcommands

| Subcommand | Purpose |
|---|---|
| `init` | Initialize config, `.env` template, scope file, DB |
| `serve` | Start API server + background worker loop |
| `agent` | Submit an agent goal |
| `scan` | Run a scoped security scan |
| `doctor` | Diagnostics on OS/RAM/disk/SQLite/env |
| `migrate` | Run DB migrations (+ `--seed`) |
| `backup` | Safe DB + workspace snapshot |

## Environment

Copy `.env.example` to `.env` and set real values. All secrets (API keys, JWT,
Stripe, webhook secrets) must come from environment variables — never committed.

See `.env.example` for the full variable reference.

## Testing

```bash
make test        # Rust tests + TS typecheck + smoke test
make clippy      # Rust clippy (deny warnings)
cargo test --manifest-path crates/wired/Cargo.toml
```

The full test suite runs offline with the built-in mock LLM provider — no live
API keys required.

## Docker

```bash
docker compose up -d
# data persisted in the wire-desk-data volume
```

## Docs

- `SECURITY.md` — authorized scope policy, bug bounty safe-harbor rules
- `API.md` — REST + SSE + webhook endpoint reference
- `RUNBOOK.md` — startup, backup/restore, incident response, scaling

## License

Apache-2.0
