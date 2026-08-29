# syntax=docker/dockerfile:1
# Local source build of the grok TUI. Host has no Rust toolchain; compile
# in Ubuntu 24.04 so the binary matches the host glibc.

FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/bin:$PATH \
    CARGO_TERM_COLOR=always \
    CARGO_INCREMENTAL=1 \
    PROTOC=/usr/local/bin/protoc

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        git \
        libclang-dev \
        libdbus-1-dev \
        pkg-config \
        unzip \
    && rm -rf /var/lib/apt/lists/*

# Match bin/protoc (DotSlash) so generated protobuf code stays in-range.
ARG PROTOC_VERSION=29.3
RUN curl -fsSL -o /tmp/protoc.zip \
        "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip" \
    && unzip -q /tmp/protoc.zip -d /usr/local \
    && rm /tmp/protoc.zip \
    && protoc --version

# rust-toolchain.toml pins 1.94.0; install that toolchain so cargo does not
# re-download it on every build.
ARG RUST_VERSION=1.94.0
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --default-toolchain "${RUST_VERSION}" --profile minimal \
    && rustc --version

WORKDIR /src
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build -p xai-grok-pager-bin --release \
    && cp /src/target/release/xai-grok-pager /tmp/grok \
    && strip /tmp/grok

FROM scratch AS export
COPY --from=builder /tmp/grok /grok
