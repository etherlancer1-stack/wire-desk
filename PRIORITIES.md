# Wire Desk — Top 15 Post-Rebuild Engineering Priorities (Output Block Y)

1. Install Rust toolchain and run `cargo build --release`; fix compile errors.
2. Run `cargo test` and `make test`; get the full offline suite green.
3. Wire real Stripe API calls into `billing.rs` (currently dev-mode skeleton).
4. Implement the full scanner engine in `crate::scanner` (worker.rs references it).
5. Complete SSE stream (`/api/v1/agents/runs/:id/stream`) with `tokio_stream`.
6. Add real API-key lookup in `auth_middleware` (currently marks all `wd_live_` as analyst).
7. Implement `GET /api/v1/metrics` Prometheus endpoint.
8. Add per-tenant quota caching to avoid repeated DB aggregation.
9. Implement E2B sandbox adapter (currently local fallback only).
10. Add OpenAPI spec generation from actual route handlers (currently static stub).
11. Add TLS certificate poster check (item 131) beyond headers.
12. Add `cargo audit` and `npm audit` to CI.
13. Add Terraform / Pulumi IaC for cloud deployment.
14. Add SSO/SAML stub (enterprise roadmap, spec #190).
15. Run a production smoke test on Render/Railway with persistent disk.
