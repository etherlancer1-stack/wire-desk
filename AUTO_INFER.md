# Wire Desk — Explicit `[AUTO-INFER]` Decision Log (Output Block X)

Every optional detail left unspecified in the brief was resolved with a
disciplined, production-grade choice. All types, routes, DB columns, env vars,
and file paths are kept consistent across all waves.

## 1. Milestone inference (items 10–11)
- Last verified milestone: core Rust + TypeScript + SQLite/Drizzle architecture
  with LLM routing and agent execution.
- In-progress feature: end-to-end multi-agent orchestration, durable job queue,
  security audit pipeline, and SaaS API/billing layer.

## 2. Repository layout (item 36)
```
crates/wired/        Rust binary + lib (Axum/Tokio/rusqlite)
src-ts/              TypeScript + Drizzle SQL schema + API client + tests
web/                 index.html, styles.css, app.js (vanilla JS dashboard)
scripts/             smoke_test, run_daemon, backup, restore, snapshot_git, pre_commit_check
migrations/          0001_initial.sql, 0002_seed.sql
```

## 3. Database engine (items 56–58)
- Primary: SQLite with WAL, `synchronous=NORMAL`, `foreign_keys=ON`,
  `busy_timeout=5000`, `temp_store=MEMORY`.
- Optional PostgreSQL/Drizzle compatibility noted but not implemented (Drizzle
  schema targets sqlite-core).

## 4. Timestamps (item 81)
- ISO-8601 UTC TEXT (`YYYY-MM-DDTHH:MM:SSZ`) stored in all `*_at` columns.
- Rust: `chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")`.

## 5. Primary keys / IDs
- UUID v4 strings for all `id` columns (`uuid` crate, `v4` feature).

## 6. LLM providers (item 86)
- OpenRouter, NVIDIA NIM, Gemini API, DeepSeek via a unified `LlmProvider` trait
  + `reqwest` HTTP; offline deterministic `MockProvider` when keys unset.

## 7. Model defaults (items 88, 93)
- `WIREDESK_MODEL_FAST=mock://offline`, `WIREDESK_MODEL_STRONG=openrouter/auto`.
- Placeholder per-1M rates: deepseek (0.14/0.28), openrouter (1.0/2.0),
  nvidia_nim (0.5/1.0), gemini (0.25/0.50).

## 8. Agent limits (items 97, 105)
- `WIREDESK_AGENT_MAX_ITERATIONS=10` (range 1–100 enforced).
- `WIREDESK_AGENT_TOOL_OUTPUT_CAP_CHARS=8000`.

## 9. Job queue (items 76–78)
- Atomic claim via `SELECT ... LIMIT 1` + `UPDATE` inside a transaction.
- Lease TTL `WIREDESK_JOB_LEASE_TTL_SECONDS=300`; stale reaper every 60s.
- Exponential backoff `2^attempt` capped at 3600s; dead-letter at `max_attempts`.

## 10. Auth (items 148, 162)
- Argon2id passwords; JWT session tokens (7-day TTL); SHA-256 hash +
  `wd_live_` prefix for API keys; raw API key shown once on creation only.

## 11. Billing tiers (item 164)
- free $0, pro $29, team $99, enterprise $499 — encoded in `Billing::plans()`
  and mirrored in `PlanTier` quota methods.

## 12. Stripe (items 166–168)
- Checkout, portal, and webhook handler; offline/dev mode accepts webhook when
  secret non-empty (real HMAC verification implemented in `verify_stripe_signature`
  skeleton; production must call Stripe API).

## 13. Server / ports (items 38–39)
- Axum on `WIREDESK_HOST`/`WIREDESK_PORT` (default `0.0.0.0:8080`).
- `/healthz`, `/readyz`, `/api/v1/status`, `/api/v1/openapi.json`, `/metrics`.

## 14. Rate limiting (items 149, 193)
- Token-bucket per IP/API key; headers `X-RateLimit-Limit/Remaining`,
  `Retry-After`; 429 on saturation.

## 15. Body limit (item 197)
- 1 MB max request body (`tower_http::limit::RequestBodyLimitLayer`).

## 16. Frontend (items 216–218)
- Vanilla JS + CSS single-page dashboard; no framework; mobile-responsive.

## 17. Docker (items 229–230)
- Multi-stage: `rust:1.78-slim` builder → `debian:bookworm-slim` runtime,
  non-root `wired` user, healthcheck, named volumes.

## 18. Testing (items 218–222)
- Rust `#[cfg(test)]` modules in db.rs, queue.rs, webhooks.rs.
- TypeScript `node --test` tests under `src-ts/tests/`.
- Offline mock provider for CI.

## 19. Concurrency guards (items 117, 240)
- `tokio::sync::Semaphore` for LLM concurrency (default 4) and worker
  concurrency (default 4); bounded caches (LRU-capacity 1024, TTL 300s).

## 20. Feature flags (item 48)
- `WIREDESK_FEATURE_EXPERIMENTAL_AGENTS`, `_SECURITY_SCANNERS`, `_PUBLIC_ROUTES`,
  `_BILLING_ENFORCEMENT` — runtime booleans.

## 21. Environment modes (item 49)
- `development`, `test`, `production`; production fails fast on weak secrets
  (<32 chars or default).
