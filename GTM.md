# Wire Desk — Go-To-Market & First $10k MRR Playbook (Output Blocks V & U)

## Phase 1 — MVP (weeks 1–4)
1. Ship CLI + API with SQLite, mock LLM, passive recon, SAST secrets scan.
2. Free public security grader at `POST /api/v1/public/security-grade`.
3. `make init && make serve` works in < 5 minutes on Linux/Termux/Docker.
4. First 10 users via Hacker News, Reddit (r/websecurity, r/SaaS), dev.to.

## Phase 2 — Beta & free grader funnel (weeks 5–8)
1. Public grader captures emails; strict rate limit (2 rps), SSRF-safe.
2. Onboarding flow: default workspace + sample scan + first agent run < 60s.
3. Referral codes at signup; track conversions.
4. Target: 100 free users, 500 grader submissions/week.

## Phase 3 — Paid conversions (weeks 9–12)
1. Stripe Checkout for Pro ($29) and Team ($99).
2. Quota enforcement: 402/429 with upgrade tier info when limits hit.
3. White-label reports for Team; outbound webhooks.
4. Target: 10 paying customers, ~$500 MRR.

## Phase 4 — Agency / enterprise (months 4–6)
1. Enterprise tier ($499+): custom quotas, dedicated sandbox, audit exports.
2. SOC2-style audit log export, GDPR data export/delete.
3. Content marketing: case studies, bug-bounty hunter outreach.
4. Target: $10k MRR.

## Channels & angles (spec #293)
- Developer communities (Hacker News, Lobste.rs, dev.to)
- Bug bounty hunters (HackerOne programs, scope-safe messaging)
- Open-source maintainers (SAST/dep-audit for their repos)
- Agency outbound (white-label security reports for clients)

## README badges & content (spec #189)
- "Scanned by Wire Desk" badge for repos
- "Audited with Wire Desk" report footer
- Case studies from early beta users
