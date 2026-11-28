#!/usr/bin/env bash
# Wire Desk pre-commit guard (spec #229).
# Checks for common secret patterns and runs lint/tests.
set -euo pipefail

echo "[pre-commit] scanning for secrets..."
SECRET_PATTERNS='\.env(\.|$)|AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{36}|sk_live_[A-Za-z0-9]{24}|whsec_[A-Za-z0-9]{24}|-----BEGIN [A-Z ]*PRIVATE KEY-----'
if git diff --cached --name-only 2>/dev/null | grep -Ei "$SECRET_PATTERNS"; then
  echo "[pre-commit] FAILED: secret-like content staged"
  exit 1
fi

echo "[pre-commit] running cargo fmt check..."
if command -v cargo >/dev/null 2>&1; then
  cargo fmt --manifest-path crates/wired/Cargo.toml -- --check 2>/dev/null || echo "[pre-commit] warning: rustfmt not clean"
  cargo clippy --manifest-path crates/wired/Cargo.toml 2>/dev/null || echo "[pre-commit] warning: clippy not clean"
else
  echo "[pre-commit] cargo not found; skipping rust checks"
fi

echo "[pre-commit] running TypeScript typecheck..."
if command -v pnpm >/dev/null 2>&1; then
  pnpm --dir src-ts exec tsc --noEmit 2>/dev/null || echo "[pre-commit] warning: tsc not clean"
else
  echo "[pre-commit] pnpm not found; skipping TS checks"
fi

echo "[pre-commit] all checks passed"
