# Harbor-compatible portable build env for bitfun-cli.
#
# Builds x86_64-unknown-linux-musl so the resulting bitfun-cli is static-pie
# linked and can run in both glibc task images (Ubuntu/Debian) and musl task
# images (Alpine).
FROM debian:bookworm

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:${PATH}

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        build-essential \
        ca-certificates \
        cmake \
        curl \
        file \
        git \
        musl-tools \
        perl \
        pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /src
