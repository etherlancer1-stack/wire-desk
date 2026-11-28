# Wire Desk — Exact Setup, Build, Migration & Test Commands (Output Block T)

## Prerequisites
- Rust: https://rustup.rs (cargo, rustc)
- Node / pnpm / bun (already present in this sandbox)
- sqlite3 (optional, for CLI inspection)

## 250 — Build commands (clean clone → compiled)
```bash
git clone https://github.com/etherlancer1/wire-desk.git
cd wire-desk
cp .env.example .env
# edit .env with real values (WIREDESK_JWT_SECRET, Stripe keys, etc.)
make build-rust        # cargo build --release
make build-ts          # npx tsc --noEmit
```

## 251 — Database init & seed
```bash
make init              # wired init --db data/wire-desk.db --scope data/scope.allowlist.json
make seed              # wired migrate --db data/wire-desk.db --seed
# or manually:
./crates/wired/target/release/wired migrate --db data/wire-desk.db --seed
```

## 252 — Run & daemon
```bash
# Foreground server
make serve
# or
./crates/wired/target/release/wired serve --host 0.0.0.0 --port 8080

# CLI agent run
./crates/wired/target/release/wired agent --goal "Summarize security posture of example.com" --json

# CLI scan
./crates/wired/target/release/wired scan --target https://example.com --type passive_recon --json

# Doctor
./crates/wired/target/release/wired doctor --db data/wire-desk.db

# Background daemon (watchdog with backoff)
bash scripts/run_daemon.sh watchdog
```

## 253 — Test & smoke verification
```bash
make test              # cargo test + tsc + smoke_test.sh
cargo test --manifest-path crates/wired/Cargo.toml
cd src-ts && npm test
bash scripts/smoke_test.sh
```

## Docker
```bash
docker compose up -d
curl http://localhost:8080/healthz
```
