# Dockerfile — Talaria API (+ embedded web/dist) for Render / any Docker host.
# Build: docker build -t talaria .
# Run:   docker run --rm -p 8080:8080 -e DATABASE_URL=... talaria

# --- web ---
FROM node:22-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# --- rust ---
FROM rust:bookworm AS rust
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
COPY fixtures ./fixtures
RUN cargo build --release -p talaria-api \
  && strip target/release/talaria

# --- runtime ---
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --create-home --uid 10001 talaria

WORKDIR /app
COPY --from=rust /app/target/release/talaria /usr/local/bin/talaria
COPY --from=web /web/dist ./web/dist
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh \
  && mkdir -p /data \
  && chown -R talaria:talaria /data /app

USER talaria
ENV TALARIA_DATA_ROOT=/data \
    RUST_LOG=talaria_api=info,tower_http=info \
    TALARIA_OFFLINE_ONLY=false \
    WIKI_LANG=en
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${PORT:-8080}/health" || exit 1
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
