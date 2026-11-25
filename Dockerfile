# syntax=docker/dockerfile:1.7
# Immagine multi-architettura (linux/amd64 + linux/arm64): si costruisce con `docker buildx`, vedi README.

# --- 1. Interfaccia web (PWA) ---------------------------------------------------------------
FROM node:22-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# --- 2. Server Rust, con database cifrato (SQLCipher + OpenSSL compilati dentro il binario) ----
FROM rust:1-bookworm AS server
RUN apt-get update && apt-get install -y --no-install-recommends perl make && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY LICENSE ./LICENSE
COPY server/ ./server/
WORKDIR /src/server
RUN cargo build --release --locked --no-default-features --features sqlcipher

# --- 3. Immagine finale: niente shell, niente gestore di pacchetti, utente non root -----------
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=server /src/server/target/release/taskhit-server /usr/local/bin/taskhit-server
COPY --from=web /web/dist /app/web
ENV TASKHIT_DATA_DIR=/data \
    TASKHIT_WEB_DIR=/app/web \
    TASKHIT_BIND=0.0.0.0:8080
VOLUME ["/data"]
EXPOSE 8080
USER 1000:1000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/usr/local/bin/taskhit-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/taskhit-server"]
