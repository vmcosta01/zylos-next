# =============================================================================
# Zylos-Next Multi-Stage Dockerfile
# Build: cargo build --release (gateway + cockpit WASM)
# Runtime: debian:bookworm-slim (~45MB final image)
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1: Rust toolchain + build dependencies
# ---------------------------------------------------------------------------
FROM rust:latest AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    protobuf-compiler \
    curl \
    libclang-dev libpipewire-0.3-dev libspa-0.2-dev libxcb1-dev libxrandr-dev \
    libdbus-1-dev libgbm-dev libegl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install wasm-pack for cockpit WASM build
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /build

# Cache dependencies: copy manifests first, then source
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

# Build gateway binary (release)
RUN cargo build --release --bin zylos-gateway

# Build cockpit WASM bundle (optional — gateway works without it)
RUN cd apps/zylos-cockpit && wasm-pack build --target web --out-dir ../../dist/cockpit --release || mkdir -p /build/dist/cockpit

# ---------------------------------------------------------------------------
# Stage 2: Minimal runtime image
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libpipewire-0.3-0 libxcb1 libxcb-randr0 libxrandr2 libdbus-1-3 libgbm1 libegl1 \
    libsqlite3-0 \
    tini \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false zylos

# Copy gateway binary
COPY --from=builder /build/target/release/zylos-gateway /usr/local/bin/zylos-gateway

# Copy cockpit static assets (WASM + JS + HTML) — may be empty if wasm-pack failed
COPY --from=builder /build/dist/cockpit/ /usr/share/zylos/cockpit/
COPY apps/zylos-cockpit/static/ /usr/share/zylos/cockpit/static/

# Create data directories
RUN mkdir -p /var/lib/zylos/data /var/lib/zylos/logs /etc/zylos \
    && chown -R zylos:zylos /var/lib/zylos /etc/zylos

# Default config
COPY <<'EOF' /etc/zylos/config.toml
[server]
listen_addr = "0.0.0.0:8080"

[database]
path = "/var/lib/zylos/data/zylos.db"

[logging]
level = "info"
format = "json"
EOF

USER zylos
# O gateway lê ZYLOS_LISTEN (padrão do binário: 127.0.0.1:3000, inacessível de fora do container)
ENV ZYLOS_LISTEN=0.0.0.0:8080
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["zylos-gateway"]