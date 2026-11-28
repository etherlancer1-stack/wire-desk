# Wire Desk — End-to-End Verification Checklist (Output Block U)

Run in order against a fresh `make init && make seed` environment.

1. [ ] `cargo build --release` succeeds with no warnings (`-D warnings`).
2. [ ] `./wired doctor --db data/wire-desk.db` reports OK (DB WAL, env, RAM).
3. [ ] `curl /healthz` → 200 `{"ok":true}`.
4. [ ] `curl /readyz` → 200 `{"ready":true}`.
5. [ ] `POST /api/v1/auth/register` → 200 + token.
6. [ ] `POST /api/v1/auth/login` → 200 + token.
7. [ ] `POST /api/v1/auth/me` with Bearer token → 200 user.
8. [ ] `POST /api/v1/workspaces` → 200 workspace; scope allowlist stored.
9. [ ] `GET /api/v1/workspaces/:id/scope` → returns allowlist.
10. [ ] SSRF block: scanner rejects `http://127.0.0.1`, `http://169.254.169.254`.
11. [ ] `POST /api/v1/scans` (passive_recon) → 200 + findings.
12. [ ] `GET /api/v1/scans/:id/findings` → findings list.
13. [ ] `PATCH /api/v1/findings/:id` `{status:"resolved"}` → updated.
14. [ ] `POST /api/v1/agents/runs` with idempotency key → no duplicate on retry.
15. [ ] `GET /api/v1/agents/runs/:id/stream` → SSE events.
16. [ ] `GET /api/v1/billing/plans` → 4 tiers.
17. [ ] Stripe webhook endpoint returns 200 (dev mode).
18. [ ] `GET /api/v1/openapi.json` → valid OpenAPI 3.1.
19. [ ] `./scripts/backup.sh` → valid tar.gz; `./scripts/restore.sh` → integrity OK.
20. [ ] `./scripts/smoke_test.sh` → all assertions pass.
21. [ ] `cargo test` + `pnpm test` → offline, green.
