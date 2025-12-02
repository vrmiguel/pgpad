# syntax=docker/dockerfile:1.7-labs

# Multi-stage build for pgpad: build frontend, bundle Tauri, export artifacts,
# and an optional Linux runtime image.

############################
# Stage 1: Frontend build  #
############################
FROM node:20-bullseye AS node-builder
WORKDIR /app

# Install deps first for better layer caching
COPY package*.json ./
RUN npm ci

# Copy full repo and build Svelte + Vite to dist/
COPY . .
RUN npm run build

#################################
# Stage 2: Tauri Rust bundling  #
#################################
FROM rust:1-bullseye AS rust-tauri-builder

# Tauri build dependencies for Linux bundles
RUN apt-get update && apt-get install -y \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libglib2.0-dev \
    libwebkit2gtk-4.1-dev \
    pkg-config \
    build-essential \
    patchelf \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Bring the full source, including dist/ from node-builder
COPY --from=node-builder /app /app

# Install Tauri CLI via cargo and build release bundles
RUN cargo install tauri-cli@^2.5.0

# Build Tauri (reads frontendDist: ../dist from tauri.conf.json)
RUN cargo tauri build

#################################
# Stage 3: Artifacts container  #
#################################
# Minimal image that only contains release bundles for easy docker cp or --output
FROM alpine:3.19 AS artifacts
WORKDIR /out

# Copy all generated bundles (AppImage/deb/rpm on Linux)
COPY --from=rust-tauri-builder /app/src-tauri/target/release/bundle /out/bundle

################################
# Stage 4: Linux runtime image #
################################
FROM debian:bookworm AS runtime

# Install runtime libraries required by Tauri/WebKit
RUN apt-get update && apt-get install -y \
    libgtk-3-0 \
    libayatana-appindicator3-1 \
    librsvg2-2 \
    libglib2.0-0 \
    libwebkit2gtk-4.1-0 \
    ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash pgpad
USER pgpad
WORKDIR /home/pgpad

# Copy built bundles
COPY --chown=pgpad:pgpad --from=rust-tauri-builder /app/src-tauri/target/release/bundle /opt/pgpad/bundle

# Entrypoint script will locate the AppImage and run it
COPY --chown=pgpad:pgpad docker/runtime-entrypoint.sh /opt/pgpad/entrypoint.sh
ENV PGPAD_DATA_DIR=/home/pgpad/.local/share/pgpad \
    PGPAD_CERT_DIR=/opt/pgpad/certs

ENTRYPOINT ["/opt/pgpad/entrypoint.sh"]

# Default command can be overridden by compose or docker run
CMD [""]
