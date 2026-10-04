# --- Build Stage Frontend ---
FROM node:22-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ ./
RUN npm run build:all

# --- Build Stage Backend ---
FROM rust:slim-bookworm AS backend-builder
WORKDIR /app/backend
RUN apt-get update && apt-get install -y pkg-config libssl-dev gcc && rm -rf /var/lib/apt/lists/*
COPY backend/Cargo.* ./
# Pre-build dependencies
RUN mkdir src && echo "pub fn dummy() {}" > src/lib.rs && echo "fn main() {}" > src/main.rs \
    && cargo build --release --bin oxid \
    && rm -rf src
COPY backend/ ./
RUN touch src/lib.rs src/main.rs && cargo build --release --bin oxid

# --- Final Lightweight Runtime Image (< 60MB) ---
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    poppler-utils \
    ca-certificates \
    fonts-liberation \
    curl \
    libheif-examples \
    && rm -rf /var/lib/apt/lists/*

# Install lightweight office2pdf binary (Rust + Typst)
ARG OFFICE2PDF_VERSION=v0.6.8
RUN curl -sSL "https://github.com/developer0hye/office2pdf/releases/download/${OFFICE2PDF_VERSION}/office2pdf-${OFFICE2PDF_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz -C /tmp \
    && mv /tmp/office2pdf-${OFFICE2PDF_VERSION}-x86_64-unknown-linux-musl/office2pdf /usr/local/bin/office2pdf \
    && chmod +x /usr/local/bin/office2pdf \
    && rm -rf /tmp/office2pdf*

COPY --from=backend-builder /app/backend/target/release/oxid /usr/local/bin/oxid
RUN ln -s /usr/local/bin/oxid /usr/local/bin/oxidrender
COPY --from=frontend-builder /app/backend/static /app/static

ENV OXID_HOST=0.0.0.0
ENV OXID_PORT=8080
ENV OXID_DATA_DIR=/data
ENV OXID_FRONTEND_DIR=/app/static

EXPOSE 8080
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/oxid"]
