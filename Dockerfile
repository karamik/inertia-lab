# Dockerfile
# Inertia Protocol — Post-Internet Digital Species
# Multi-stage build for small image size

# ============ Stage 1: Builder ============
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libudev-dev \
    libasound2-dev \
    clang \
    llvm \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source files to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "#[cfg(test)] mod tests {}" > src/lib.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src
COPY inertia-core ./inertia-core
COPY inertia-transport ./inertia-transport
COPY inertia-consensus ./inertia-consensus
COPY inertia-storage ./inertia-storage
COPY inertia-crypto ./inertia-crypto
COPY inertia-network ./inertia-network
COPY inertia-client ./inertia-client

# Build with all features
RUN cargo build --release --features full

# ============ Stage 2: Runtime ============
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libudev1 \
    libasound2 \
    bluez \
    bluez-tools \
    alsa-utils \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/inertiad /usr/local/bin/inertiad

# Copy scripts
COPY scripts/entrypoint.sh /entrypoint.sh
COPY scripts/healthcheck.sh /healthcheck.sh
RUN chmod +x /entrypoint.sh /healthcheck.sh

# Create data directory
RUN mkdir -p /data
VOLUME ["/data"]

# Expose ports
EXPOSE 18888 18889 9090

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD /healthcheck.sh

# Entry point
ENTRYPOINT ["/entrypoint.sh"]
CMD ["start", "--name", "inertia-docker", "--bluetooth", "--wifi-stego"]
