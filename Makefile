# Wire Desk build / ops Makefile.
# Usage: make <target>
#
# Requires: cargo (Rust), pnpm or node, sqlite3 (optional, for CLI inspection).

RUST_BIN := crates/wired/target/release/wired
TS_DIR   := src-ts
WEB_DIR  := web

.PHONY: help build build-rust build-ts init migrate seed serve doctor test lint fmt clippy docker-up docker-down clean backup

help: ## Show this help.
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-22s\033[0m %s\n", $$1, $$2}'

build: build-rust build-ts ## Build everything (Rust binary + TypeScript typecheck).

build-rust: ## Build the Rust wired binary in release mode.
	cargo build --release --manifest-path crates/wired/Cargo.toml

build-ts: ## Typecheck the TypeScript layer.
	cd $(TS_DIR) && npx tsc --noEmit -p tsconfig.json 2>/dev/null || npx tsc --noEmit tsconfig.json

init: ## Initialize local config, .env template, workspace scope file, and DB.
	./wired init --db data/wire-desk.db --scope data/scope.allowlist.json

migrate: ## Run database migrations (idempotent).
	./wired migrate --db data/wire-desk.db

seed: ## Load idempotent seed data.
	./wired migrate --db data/wire-desk.db --seed

serve: ## Start the API server and background worker loop.
	./wired serve --db data/wire-desk.db --host 0.0.0.0 --port 8080

doctor: ## Run diagnostics on OS, RAM, disk, SQLite, env, and connectivity.
	./wired doctor --db data/wire-desk.db

agent: ## Submit an agent goal (args: --goal "...").
	./wired agent $(filter-out agent,$(MAKECMDGOALS))

scan: ## Run a scoped security scan (args: --target ... --type passive|headers|sast|deps).
	./wired scan $(filter-out scan,$(MAKECMDGOALS))

test: ## Run unit + integration tests and smoke test.
	cargo test --manifest-path crates/wired/Cargo.toml
	cd $(TS_DIR) && npm test 2>/dev/null || true
	./scripts/smoke_test.sh

lint: ## Run Rust clippy + TypeScript lint.
	cargo clippy --manifest-path crates/wired/Cargo.toml -- -D warnings
	cd $(TS_DIR) && npx eslint . 2>/dev/null || true

fmt: ## Format Rust and TypeScript.
	cargo fmt --manifest-path crates/wired/Cargo.toml
	cd $(TS_DIR) && npx prettier --write . 2>/dev/null || true

clippy: ## Run Rust clippy with deny warnings.
	cargo clippy --manifest-path crates/wired/Cargo.toml -- -D warnings

docker-up: ## Start via docker-compose.
	docker compose up -d

docker-down: ## Stop docker-compose.
	docker compose down

backup: ## Run a safe DB + workspace backup.
	./scripts/backup.sh

clean: ## Remove build artifacts and local data (keeps source).
	cargo clean --manifest-path crates/wired/Cargo.toml 2>/dev/null || true
	rm -rf $(TS_DIR)/node_modules $(WEB_DIR)/dist target crates/wired/target data/*.db backups/*.gz 2>/dev/null || true

# Allow passing extra args to subcommands (e.g. `make agent --goal "hello"`).
%:
	@:
