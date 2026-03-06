# Multi-stage build for BitFun Relay Server
FROM rust:1.82-slim AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy all workspace Cargo.toml files (including nested path deps in bitfun-core)
COPY Cargo.toml Cargo.lock ./
COPY src/crates/events/Cargo.toml src/crates/events/Cargo.toml
COPY src/crates/core/Cargo.toml src/crates/core/Cargo.toml
COPY src/crates/core/src/infrastructure/ai/ai_stream_handlers/Cargo.toml src/crates/core/src/infrastructure/ai/ai_stream_handlers/Cargo.toml
COPY src/crates/core/src/agentic/tools/implementations/tool-runtime/Cargo.toml src/crates/core/src/agentic/tools/implementations/tool-runtime/Cargo.toml
COPY src/crates/core/src/service/terminal/Cargo.toml src/crates/core/src/service/terminal/Cargo.toml
COPY src/crates/transport/Cargo.toml src/crates/transport/Cargo.toml
COPY src/crates/api-layer/Cargo.toml src/crates/api-layer/Cargo.toml
COPY src/apps/cli/Cargo.toml src/apps/cli/Cargo.toml
COPY src/apps/desktop/Cargo.toml src/apps/desktop/Cargo.toml
COPY src/apps/server/Cargo.toml src/apps/server/Cargo.toml
COPY src/apps/relay-server/Cargo.toml src/apps/relay-server/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p src/crates/events/src && echo "pub fn dummy() {}" > src/crates/events/src/lib.rs && \
    mkdir -p src/crates/core/src && echo "pub fn dummy() {}" > src/crates/core/src/lib.rs && \
    mkdir -p src/crates/core/src/infrastructure/ai/ai_stream_handlers/src && echo "pub fn dummy() {}" > src/crates/core/src/infrastructure/ai/ai_stream_handlers/src/lib.rs && \
    mkdir -p src/crates/core/src/agentic/tools/implementations/tool-runtime/src && echo "pub fn dummy() {}" > src/crates/core/src/agentic/tools/implementations/tool-runtime/src/lib.rs && \
    mkdir -p src/crates/core/src/service/terminal/src && echo "pub fn dummy() {}" > src/crates/core/src/service/terminal/src/lib.rs && \
    mkdir -p src/crates/transport/src && echo "pub fn dummy() {}" > src/crates/transport/src/lib.rs && \
    mkdir -p src/crates/api-layer/src && echo "pub fn dummy() {}" > src/crates/api-layer/src/lib.rs && \
    mkdir -p src/apps/cli/src && echo "fn main() {}" > src/apps/cli/src/main.rs && \
    mkdir -p src/apps/desktop/src && echo "fn main() {}" > src/apps/desktop/src/main.rs && \
    mkdir -p src/apps/server/src && echo "fn main() {}" > src/apps/server/src/main.rs && \
    mkdir -p src/apps/relay-server/src && echo "pub fn dummy() {}" > src/apps/relay-server/src/lib.rs && \
    echo "fn main() {}" > src/apps/relay-server/src/main.rs

# Build dependencies only (cached layer)
RUN cargo build --release -p bitfun-relay-server 2>/dev/null || true

# Copy actual source code
COPY src/apps/relay-server/src src/apps/relay-server/src

# Build the relay server
RUN cargo build --release -p bitfun-relay-server

# ── Runtime stage ─────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/bitfun-relay-server /app/bitfun-relay-server

COPY src/mobile-web/dist /app/static

ENV RELAY_PORT=9700
ENV RELAY_STATIC_DIR=/app/static
EXPOSE 9700

CMD ["/app/bitfun-relay-server"]
