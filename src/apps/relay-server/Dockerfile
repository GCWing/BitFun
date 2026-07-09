# BitFun Relay Server — standalone Docker build.
# Build context is relay-server root (Cargo.toml + src/), no workspace needed.
FROM rust:1-slim AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
RUN mkdir -p src/bin && echo 'fn main() { println!("placeholder"); }' > src/main.rs && echo 'fn main() { println!("placeholder"); }' > src/bin/relay_admin.rs
RUN cargo build --release 2>/dev/null || true

RUN rm -rf src target/release/bitfun-relay-server target/release/relay-admin target/release/deps/bitfun*

COPY src/ src/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/bitfun-relay-server /app/bitfun-relay-server
COPY --from=builder /build/target/release/relay-admin /app/relay-admin
RUN mkdir -p /app/static

ENV RELAY_PORT=9700
ENV RELAY_STATIC_DIR=/app/static
EXPOSE 9700

CMD ["/app/bitfun-relay-server"]
