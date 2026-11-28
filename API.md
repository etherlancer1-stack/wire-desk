# Wire Desk API Reference

All responses use the consistent envelope (spec #194):

Success: `{ "ok": true, "data": {...}, "request_id": "..." }`
Error:   `{ "ok": false, "error": { "code": "...", "message": "...", "details": ... }, "request_id": "..." }`

Error codes: `UNAUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `VALIDATION_ERROR`,
`SCOPE_VIOLATION`, `QUOTA_EXCEEDED`, `RATE_LIMITED`, `INTERNAL_ERROR`,
`PAYMENT_REQUIRED` (spec #295).

## Auth (`POST /api/v1/auth/register`, `POST /api/v1/auth/login`, `GET /api/v1/auth/me`)

- `POST /api/v1/auth/register` — `{ email, password, tenant_name?, tenant_slug? }` → `{ token, user_id, tenant_id, role }`
- `POST /api/v1/auth/login` — `{ email, password }` → `{ token, ... }`
- `GET /api/v1/auth/me` — Bearer JWT → current user

Auth via `Authorization: Bearer <jwt>` or `Authorization: Bearer wd_live_<api_key>`.

## API keys (`POST|GET /api/v1/keys`, `DELETE /api/v1/keys/:id`)

- `POST /api/v1/keys` — create scoped API key (raw key shown once)
- `GET /api/v1/keys` — list prefixes/metadata (hash never returned)
- `DELETE /api/v1/keys/:id` — revoke immediately

## Workspaces (`POST|GET /api/v1/workspaces`, `GET /:id`, `PATCH /:id/scope`)

- `POST` — `{ name, description?, scope_allowlist?, rate_limit_rps? }`
- `PATCH /:id/scope` — update `scope_allowlist_json` + `rate_limit_rps`
- Every query enforces `tenant_id` filtering (spec #161).

## Agent runs (`POST|GET /api/v1/agents/runs`, `GET /:id`, `POST /:id/cancel`, `GET /:id/stream`)

- `POST` — `{ goal, workspace_id, idempotency_key? }` (idempotent submission, spec #111)
- `GET /:id/stream` — Server-Sent Events: `step_started`, `tool_output`, `token_delta`, `run_completed`, `run_failed` (spec #108)

## Security scans (`POST|GET /api/v1/scans`, `GET /:id`, `GET /:id/findings`, `PATCH /api/v1/findings/:id`, `GET /:id/report`)

- `POST` — `{ target, workspace_id, scan_type }` (`passive_recon`, `dns_tls_audit`, `sast_repo`, `dep_audit`, `ssrf_check`)
- `PATCH /api/v1/findings/:id` — `{ status: open|triaged|resolved|false_positive }` (spec #155)
- `GET /:id/report` — bug-bounty or white-label report (specs #141, #172)

## Billing (`GET /api/v1/billing/plans`, `GET /api/v1/billing/usage`, `POST /api/v1/billing/checkout`, `POST /api/v1/billing/portal`, `POST /api/v1/webhooks/stripe`)

- `POST /api/v1/billing/checkout` — Stripe Checkout session with `tenant_id` metadata (#166)
- `POST /api/v1/billing/portal` — Stripe Customer Portal (#167)
- `POST /api/v1/webhooks/stripe` — Stripe webhook handler; idempotent by event ID (#168)

## Webhooks & admin

- `POST|GET /api/v1/webhooks` — outbound signed webhook endpoints (#174)
- `GET /api/v1/audit-logs` — immutable audit trail (#184)
- `GET /api/v1/admin/overview` — founder analytics (#178)
- `GET /api/v1/export` — GDPR data export (#182)

## Public grader (`POST /api/v1/public/security-grade`)

No-auth, strictly rate-limited, SSRF-safe passive header/DNS/TLS grader (#179).

## Status & observability

- `GET /healthz` — liveness (#176)
- `GET /readyz` — DB + config readiness (#176)
- `GET /api/v1/status` — service status (#176)
- `GET /api/v1/openapi.json` — OpenAPI 3.1 (#175)
- `GET /metrics` — Prometheus metrics (#177)
- `GET /api/v1/changelog` — in-app changelog (#185)

## Response headers

- `X-Request-Id` on every response (#196)
- `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `Retry-After` (#193)
- `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Cache-Control: no-store` (#150)

## Cursor pagination

`GET ?limit=20&cursor=...` with `next_cursor` + `has_more` (#195).
