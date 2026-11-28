# Wire Desk — Cost & Unit Economics Model (Output Block W)

## Infrastructure costs per month (estimated, single region, low-RAM)

| Item | Cost/mo | Notes |
|------|---------|-------|
| SQLite host (shared 512MB–2GB VM) | $5–$20 | One VM can host many tenants |
| Storage (backups, logs) | $2–$5 | Bounded, pruned |
| Outbound HTTP / DNS fetches | $1–$5 | Passive recon only |
| Monitoring / metrics | $0–$5 | Self-hosted Prometheus |

## LLM routing cost per request (per-1M placeholder rates)

| Model class | Example | In/Out per 1M | Est. cost/run (≈1k in, 500 out) |
|-------------|---------|---------------|--------------------------------|
| Fast | deepseek-chat | $0.14 / $0.28 | ≈$0.0003 |
| Strong | openrouter/auto | $1.00 / $2.00 | ≈$0.002 |

## Per-tier economics (target gross margin >70%)

| Tier | Price/mo | Est. LLM cost/mo (at quota) | Infra alloc | Gross margin |
|------|----------|----------------------------|-------------|--------------|
| Free | $0 | ~$0.015 (50 runs) | $2 | negative (lead magnet) |
| Pro | $29 | ~$2.10 (1,500 runs) | $3 | ~93% |
| Team | $99 | ~$10.50 (7,500 runs) | $5 | ~89% |
| Enterprise | $499 | custom | $20+ | ~90%+ |

## Sensitivity (spec #292)
- Switching all strong-model runs to fast models cuts LLM cost ~6x.
- Margin stays >70% as long as LLM cost < 25% of tier revenue.

## Pricing page data (spec #191)
Consumable by API (`GET /api/v1/billing/plans`) and frontend UI.
