# ============================================================================
# Stage 1: Build
# ============================================================================
FROM rust:1.95-slim AS builder

WORKDIR /app

# Install system dependencies required for compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first for optimal layer caching
COPY Cargo.toml ./
COPY crates ./crates
COPY src ./src

# Build all workspace crates in release mode
# Using --workspace ensures all crates compile and links correctly
RUN cargo build --release --workspace

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN groupadd -r cbtpro && useradd -r -g cbtpro cbtpro

# Copy the compiled binary from builder
# The api crate produces the HTTP/WebSocket gateway binary
COPY --from=builder /app/target/release/cbt-pro-api /usr/local/bin/cbt-pro-api

# Set proper ownership
RUN chown cbtpro:cbtpro /usr/local/bin/cbt-pro-api

# Switch to non-root user
USER cbtpro

# Expose REST API and WebSocket ports
EXPOSE 8080
EXPOSE 8081

# Healthcheck: REST health endpoint must respond with HTTP 200
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD wget -qO- http://localhost:8080/health || exit 1

# Run the API gateway
CMD ["cbt-pro-api"]
