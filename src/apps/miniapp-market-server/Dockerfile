# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS web-builder

WORKDIR /build
RUN corepack enable && corepack prepare pnpm@10.15.0 --activate

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml .npmrc ./
COPY src/miniapp-market-web/package.json src/miniapp-market-web/package.json
RUN --mount=type=cache,id=bitfun-market-pnpm,target=/root/.local/share/pnpm/store,sharing=locked \
    pnpm install --frozen-lockfile --ignore-scripts --filter bitfun-miniapp-market-web

COPY src/miniapp-market-web/ src/miniapp-market-web/
RUN pnpm --dir src/miniapp-market-web build

FROM rust:1.96-bookworm AS rust-builder

WORKDIR /build
ARG CARGO_BUILD_JOBS=
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_TERM_COLOR=always

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN --mount=type=cache,id=bitfun-market-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=bitfun-market-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=bitfun-market-target,target=/build/target,sharing=locked \
    { [ -n "${CARGO_BUILD_JOBS:-}" ] || unset CARGO_BUILD_JOBS; } \
    && cargo build --locked --release -p bitfun-miniapp-market-server \
    && cp target/release/bitfun-miniapp-market-server /tmp/bitfun-miniapp-market-server \
    && strip /tmp/bitfun-miniapp-market-server

FROM debian:bookworm-slim AS runtime

ARG MARKET_GIT_COMMIT=unknown
LABEL org.opencontainers.image.title="BitFun MiniApp Market" \
      org.opencontainers.image.revision="${MARKET_GIT_COMMIT}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 market \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin market

WORKDIR /app
COPY --from=rust-builder /tmp/bitfun-miniapp-market-server /app/bitfun-miniapp-market-server
COPY --from=web-builder /build/src/miniapp-market-web/dist/ /app/web/

RUN mkdir -p /app/data /app/artifacts \
    && chown -R market:market /app/data /app/artifacts

USER 10001:10001

ENV MARKET_BIND=0.0.0.0:9710 \
    MARKET_DATABASE_PATH=/app/data/market.sqlite \
    MARKET_ARTIFACT_DIR=/app/artifacts \
    MARKET_WEB_DIR=/app/web

EXPOSE 9710

HEALTHCHECK --interval=20s --timeout=5s --start-period=15s --retries=5 \
  CMD curl -fsS http://127.0.0.1:9710/miniapp/api/v1/health || exit 1

ENTRYPOINT ["/app/bitfun-miniapp-market-server"]
