# Wire Desk — multi-stage Dockerfile (spec #229).
# Lean builder + lean runtime image running as non-root with healthcheck.
# Build: docker build -t wire-desk .

# ---------- Builder ----------
FROM rust:1.78-slim AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml crates/wired/Cargo.toml ./
RUN mkdir -p crates/wired/src && \
  printf 'fn main() {}\n' > crates/wired/src/main.rs && \
  printf 'pub fn _placeholder() {}\n' > crates/wired/src/lib.rs && \
  cargo build --release --manifest-path crates/wired/Cargo.toml 2>/dev/null || true
COPY crates/wired/src ./crates/wired/src
COPY migrations ./migrations
RUN cargo build --release --manifest-path crates/wired/Cargo.toml

# ---------- Runtime ----------
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates sqlite3 curl && rm -rf /var/lib/apt/lists/*
# Non-root user (spec #229).
RUN useradd --create-home --shell /bin/bash wired
COPY --from=builder /build/crates/wired/target/release/wired /usr/local/bin/wired
COPY .env.example ./
COPY web ./web
COPY scripts ./scripts
RUN chmod +x /usr/local/bin/wired /app/scripts/*.sh
RUN chown -R wired:wired /app
USER wired

ENV WIREDESK_ENV=production \
    WIREDESK_HOST=0.0.0.0 \
    WIREDESK_PORT=8080 \
    WIREDESK_DB_PATH=/app/data/wire-desk.db \
    WIREDESK_DATA_DIR=/app/data \
    WIREDESK_LOG_DIR=/app/logs \
    WIREDESK_BACKUP_DIR=/app/backups \
    WIREDESK_TMP_DIR=/app/tmp \
    WIREDESK_LOG_LEVEL=info \
    WIREDESK_LOG_FORMAT=json

VOLUME ["/app/data", "/app/backups", "/app/logs"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -sf http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/wired", "serve", "--host", "0.0.0.0", "--port", "8080"]
