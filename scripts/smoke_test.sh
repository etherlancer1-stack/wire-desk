#!/usr/bin/env bash
# Wire Desk smoke test (spec #223).
# Boots the service against a temp DB, verifies health, auth, workspace,
# security scan, agent run, billing usage, and report export, then cleans up.
set -euo pipefail

WIRED_BIN="${WIRED_BIN:./crates/wired/target/release/wired}"
DB="${DB:-$(mktemp -d)/wire-desk-smoke.db}"
PORT="${PORT:-18080}"
BASE="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0

assert() {
  if [ "$1" = "$2" ]; then
    echo "ok: $3"
    PASS=$((PASS + 1))
  else
    echo "FAIL: $3 (expected [$2] got [$1])"
    FAIL=$((FAIL + 1))
  fi
}

cleanup() {
  if [ -n "${SERVER_PID:-}" ]; then kill "$SERVER_PID" 2>/dev/null || true; fi
  rm -f "$DB" "$DB-wal" "$DB-shm"
}
trap cleanup EXIT

echo "== Wire Desk smoke test =="
echo "binary: $WIRED_BIN"
echo "db: $DB"

# Build if missing
if [ ! -x "$WIRED_BIN" ]; then
  echo "building wired..."
  cargo build --release --manifest-path crates/wired/Cargo.toml
fi

# Init + migrate
"$WIRED_BIN" init --db "$DB" --scope /tmp/wired-smoke-scope.json --seed >/dev/null 2>&1 || true

# Start server
WIREDESK_DB_PATH="$DB" WIREDESK_PORT="$PORT" WIREDESK_HOST=127.0.0.1 WIREDESK_ENV=test WIREDESK_OFFLINE_MOCK=true WIREDESK_BILLING_ENFORCEMENT=false \
  "$WIRED_BIN" serve --host 127.0.0.1 --port "$PORT" &
SERVER_PID=$!

# Wait for health
for i in $(seq 1 30); do
  if curl -sf "$BASE/healthz" >/dev/null 2>&1; then break; fi
  sleep 0.5
done
if ! curl -sf "$BASE/healthz" >/dev/null 2>&1; then
  echo "FAIL: server did not become healthy"; exit 1
fi
echo "ok: healthz"

# Readiness
CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/readyz")
assert "$CODE" "200" "readyz"

# Public grader (no auth)
CODE=$(curl -s -o /tmp/grade.json -w "%{http_code}" -X POST "$BASE/api/v1/public/security-grade" -H "Content-Type: application/json" -d '{"target":"https://example.com"}')
assert "$CODE" "200" "public security grader"

# Register
CODE=$(curl -s -o /tmp/reg.json -w "%{http_code}" -X POST "$BASE/api/v1/auth/register" -H "Content-Type: application/json" -d '{"email":"smoke@wire-desk.test","password":"smoke-password-123","tenant_name":"Smoke Tenant","tenant_slug":"smoke"}')
assert "$CODE" "200" "register"

# Login
CODE=$(curl -s -o /tmp/login.json -w "%{http_code}" -X POST "$BASE/api/v1/auth/login" -H "Content-Type: application/json" -d '{"email":"smoke@wire-desk.test","password":"smoke-password-123"}')
assert "$CODE" "200" "login"
TOKEN=$(python3 -c 'import json,sys; print(json.load(open("/tmp/login.json")).get("data",{}).get("token",""))' 2>/dev/null || echo "")
if [ -z "$TOKEN" ]; then
  # Fallback: try envelope shape
  TOKEN=$(python3 -c 'import json; d=json.load(open("/tmp/login.json")); print(d.get("data",{}).get("token","") or d.get("token",""))' 2>/dev/null || echo "")
fi
AUTH_H=(-H "Authorization: Bearer $TOKEN")

# Me
CODE=$(curl -s -o /tmp/me.json -w "%{http_code}" "${AUTH_H[@]}" "$BASE/api/v1/auth/me")
assert "$CODE" "200" "auth me"

# Create workspace
CODE=$(curl -s -o /tmp/ws.json -w "%{http_code}" "${AUTH_H[@]}" -X POST "$BASE/api/v1/workspaces" -H "Content-Type: application/json" -d '{"name":"Smoke Workspace","description":"smoke test workspace","scope_allowlist":{"domains":["example.com"],"wildcard_domains":["*.example.com"],"cidrs":[],"ips":[],"paths":[],"allow_private_ips":false},"rate_limit_rps":5}')
assert "$CODE" "200" "create workspace"
WS_ID=$(python3 -c 'import json; d=json.load(open("/tmp/ws.json")); print((d.get("data") or {}).get("id",""))' 2>/dev/null || echo "")
[ -z "$WS_ID" ] && WS_ID=$(python3 -c 'import json; d=json.load(open("/tmp/ws.json")); print(d.get("id",""))' 2>/dev/null || echo "")

# Create scan
CODE=$(curl -s -o /tmp/scan.json -w "%{http_code}" "${AUTH_H[@]}" -X POST "$BASE/api/v1/scans" -H "Content-Type: application/json" -d "{\"target\":\"https://example.com\",\"workspace_id\":\"$WS_ID\",\"scan_type\":\"passive_recon\"}")
assert "$CODE" "200" "create scan"

# List scans
CODE=$(curl -s -o /tmp/scans.json -w "%{http_code}" "${AUTH_H[@]}" "$BASE/api/v1/scans")
assert "$CODE" "200" "list scans"

# Create agent run
CODE=$(curl -s -o /tmp/run.json -w "%{http_code}" "${AUTH_H[@]}" -X POST "$BASE/api/v1/agents/runs" -H "Content-Type: application/json" -d "{\"goal\":\"Smoke test agent goal\",\"workspace_id\":\"$WS_ID\",\"idempotency_key\":\"smoke_run_$(date +%s)\"}")
assert "$CODE" "200" "create agent run"

# Billing usage
CODE=$(curl -s -o /tmp/usage.json -w "%{http_code}" "${AUTH_H[@]}" "$BASE/api/v1/billing/usage")
assert "$CODE" "200" "billing usage"

# Plans
CODE=$(curl -s -o /tmp/plans.json -w "%{http_code}" "$BASE/api/v1/billing/plans")
assert "$CODE" "200" "billing plans"

# Export
CODE=$(curl -s -o /tmp/export.json -w "%{http_code}" "${AUTH_H[@]}" "$BASE/api/v1/export")
assert "$CODE" "200" "tenant export"

# OpenAPI
CODE=$(curl -s -o /tmp/openapi.json -w "%{http_code}" "$BASE/api/v1/openapi.json")
assert "$CODE" "200" "openapi spec"

echo
echo "== results: $PASS passed, $FAIL failed =="
if [ "$FAIL" -gt 0 ]; then exit 1; fi
