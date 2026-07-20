# BitFun Relay Server standalone Docker build.
# The reduced repository-root context contains only the relay app and services.

# Pin builder to bookworm so the linked glibc matches the runtime image.
FROM rust:1-bookworm AS builder

WORKDIR /build/src/apps/relay-server

# Optional: limit rustc parallelism on small or low-memory VPS hosts.
#   docker compose build --build-arg CARGO_BUILD_JOBS=1
# Note: empty value must NOT be set as ENV — cargo chokes on empty string.
ARG CARGO_BUILD_JOBS=
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        python3 \
    && rm -rf /var/lib/apt/lists/*

COPY src/apps/relay-server/Cargo.toml ./Cargo.toml
COPY src/crates/services/relay-service/Cargo.toml ../../crates/services/relay-service/Cargo.toml
COPY src/crates/services/page-function-runtime/Cargo.toml ../../crates/services/page-function-runtime/Cargo.toml

# Build placeholders first so unchanged dependencies remain cached.
# BuildKit exposes ARG as an env during RUN; cargo rejects CARGO_BUILD_JOBS="".
RUN mkdir -p src/bin \
        ../../crates/services/relay-service/src \
        ../../crates/services/page-function-runtime/src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf 'pub use bitfun_relay_service::*;\n' > src/lib.rs \
    && printf 'fn main() {}\n' > src/bin/relay_admin.rs \
    && printf '// placeholder\n' > ../../crates/services/relay-service/src/lib.rs \
    && printf '// placeholder\n' > ../../crates/services/page-function-runtime/src/lib.rs \
    && { [ -n "${CARGO_BUILD_JOBS:-}" ] || unset CARGO_BUILD_JOBS; } \
    && cargo build --release

# Cargo emits libbitfun_*-*.rlib/.rmeta — a bare crate-name glob misses those
# and leaves the empty placeholder crate for the real rebuild.
RUN rm -rf src \
        ../../crates/services/relay-service/src \
        ../../crates/services/page-function-runtime/src \
    target/release/bitfun-relay-server \
    target/release/relay-admin \
    target/release/deps/*bitfun_relay_service* \
    target/release/deps/*bitfun_page_function_runtime* \
    target/release/deps/*bitfun_relay_server* \
    target/release/deps/*relay_admin* \
    target/release/.fingerprint/bitfun-relay-service-* \
    target/release/.fingerprint/bitfun-page-function-runtime-* \
    target/release/.fingerprint/bitfun-relay-server-* \
    target/release/.fingerprint/relay-admin-*

COPY src/apps/relay-server/src/ ./src/
COPY src/crates/services/relay-service/src/ ../../crates/services/relay-service/src/
COPY src/crates/services/page-function-runtime/src/ ../../crates/services/page-function-runtime/src/

RUN touch ../../crates/services/relay-service/src/lib.rs \
        ../../crates/services/page-function-runtime/src/lib.rs \
        src/lib.rs \
        src/main.rs \
        src/bin/relay_admin.rs \
    && { [ -n "${CARGO_BUILD_JOBS:-}" ] || unset CARGO_BUILD_JOBS; } \
    && cargo build --release \
    && (strip target/release/bitfun-relay-server target/release/relay-admin || true)

FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/src/apps/relay-server/target/release/bitfun-relay-server /app/bitfun-relay-server
COPY --from=builder /build/src/apps/relay-server/target/release/relay-admin /app/relay-admin
RUN mkdir -p /app/static /app/data /app/room-web

ENV RELAY_PORT=9700
ENV RELAY_STATIC_DIR=/app/static
EXPOSE 9700

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
  CMD curl -fsS "http://127.0.0.1:${RELAY_PORT}/health" || exit 1

CMD ["/app/bitfun-relay-server"]
