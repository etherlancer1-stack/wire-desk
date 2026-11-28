# Wire Desk — Security Policy

## Authorized scope policy

Wire Desk is a **defensive, authorized-scope auditing tool only**. Before any
network or target-specific check, the **Pre-Execution Scope Gate** verifies the
target against the workspace `scope_allowlist_json` (domains, wildcard
subdomains, CIDRs/IPs, explicit paths). Out-of-scope targets are denied and
logged (specs #121-122).

## Safe-harbor rules

- No destructive payloads or exploitation.
- No DoS flooding or denial-of-service.
- No testing outside authorized bug-bounty scope or your own assets.
- Respect the scope published by each HackerOne / bug-bounty program.

## Vulnerability reporting

Report security issues to the maintainers via the repository security advisory
process. Include:
1. Title and severity
2. Affected asset and scope verification
3. Steps to reproduce
4. Impact analysis
5. Remediation suggestion

Wire Desk can generate structured bug-bounty reports (Title, Severity, Affected
Asset, Scope Verification, Summary, Steps to Reproduce, Impact, Remediation)
via `GET /api/v1/scans/:id/report` (spec #141).

## Hardening checklist

- [ ] SQLite WAL + `foreign_keys=ON` + `busy_timeout=5000`
- [ ] SSRF IP/DNS validator blocks private/metadata ranges (spec #125)
- [ ] Path canonicalization guard: `canonical_path.starts_with(root)` (#145)
- [ ] No `sh -c` with untrusted input; explicit arg arrays only (#144)
- [ ] Secret redaction in logs/traces/reports (#146)
- [ ] HMAC-SHA256 webhook signature verification (#147)
- [ ] Argon2id passwords; SHA-256 + prefix API keys (#148)
- [ ] Sliding-window rate limiting (#149)
- [ ] Strict CORS + security response headers (#150)
- [ ] Fail-fast on weak/default secrets in production (#200)
- [ ] `413 Payload Too Large` body limit (#197)

## Legal

Wire Desk includes a safe-use acknowledgment flag
(`WIREDESK_AUTHORIZED_USE_ACKNOWLEDGED`) that must be acknowledged before
running active/external checks (spec #152).
