# BitFun Relay Server — standalone Docker build.
# Build context is relay-server root (Cargo.toml + src/), no workspace needed.
#
# Multi-arch: build natively on the deploy host (linux/amd64 or linux/arm64).
# Do not force --platform unless you intentionally cross-build with qemu.

FROM rust:1-slim AS builder

WORKDIR /build

# Optional: limit rustc parallelism on small / low-memory VPS (esp. arm64).
#   docker compose build --build-arg CARGO_BUILD_JOBS=1
ARG CARGO_BUILD_JOBS=
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        build-essential \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
RUN mkdir -p src/bin \
    && echo 'fn main() { println!("placeholder"); }' > src/main.rs \
    && echo 'fn main() { println!("placeholder"); }' > src/bin/relay_admin.rs
# Dependency cache layer (placeholder sources). Ignore failure so a toolchain
# mismatch does not abort before real sources are copied.
RUN cargo build --release 2>/dev/null || true

RUN rm -rf src target/release/bitfun-relay-server target/release/relay-admin target/release/deps/bitfun*

COPY src/ src/

RUN cargo build --release \
    && strip target/release/bitfun-relay-server target/release/relay-admin || true

FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/bitfun-relay-server /app/bitfun-relay-server
COPY --from=builder /build/target/release/relay-admin /app/relay-admin
RUN mkdir -p /app/static /app/data /app/room-web

ENV RELAY_PORT=9700
ENV RELAY_STATIC_DIR=/app/static
EXPOSE 9700

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
  CMD curl -fsS "http://127.0.0.1:${RELAY_PORT}/health" || exit 1

CMD ["/app/bitfun-relay-server"]
