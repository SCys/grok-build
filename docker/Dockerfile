# syntax=docker/dockerfile:1
# Local source build of the grok TUI. Host has no Rust toolchain; compile
# in Ubuntu 24.04 so the binary matches the host glibc.
#
# China mirrors (override with --build-arg):
#   UBUNTU_MIRROR  tuna apt
#   RUSTUP_*       rsproxy rustup + dist
#   GH_PROXY       prefix for github.com (releases + git deps)
# Host https_proxy is forwarded by scripts/docker-build-install.sh.

FROM ubuntu:24.04 AS builder

ARG UBUNTU_MIRROR=http://mirrors.tuna.tsinghua.edu.cn/ubuntu
ARG GH_PROXY=https://ghfast.top/
ARG RUSTUP_DIST_SERVER=https://rsproxy.cn
ARG RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
ARG RUSTUP_INIT_URL=https://rsproxy.cn/rustup-init.sh
ARG http_proxy
ARG https_proxy
ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG no_proxy=localhost,127.0.0.1,rsproxy.cn,mirrors.tuna.tsinghua.edu.cn,mirrors.aliyun.com,mirrors.ustc.edu.cn
ARG NO_PROXY=localhost,127.0.0.1,rsproxy.cn,mirrors.tuna.tsinghua.edu.cn,mirrors.aliyun.com,mirrors.ustc.edu.cn

# Do not put http(s)_proxy in ENV yet: the ubuntu image has no CA bundle, so a
# TLS-intercepting proxy (or HTTPS apt) fails before ca-certificates is installed.
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/bin:$PATH \
    CARGO_TERM_COLOR=always \
    CARGO_INCREMENTAL=1 \
    PROTOC=/usr/local/bin/protoc \
    RUSTUP_TOOLCHAIN=1.94.0 \
    RUST_MIN_STACK=16777216 \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    RUSTUP_DIST_SERVER=${RUSTUP_DIST_SERVER} \
    RUSTUP_UPDATE_ROOT=${RUSTUP_UPDATE_ROOT}

# Rewrite Ubuntu archive/security to a China mirror before the first apt-get.
RUN set -eux; \
    if [ -f /etc/apt/sources.list.d/ubuntu.sources ]; then \
      sed -i \
        -e "s|https://archive.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        -e "s|http://archive.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        -e "s|https://security.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        -e "s|http://security.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        /etc/apt/sources.list.d/ubuntu.sources; \
    fi; \
    if [ -f /etc/apt/sources.list ]; then \
      sed -i \
        -e "s|https://archive.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        -e "s|http://archive.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        -e "s|https://security.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        -e "s|http://security.ubuntu.com/ubuntu|${UBUNTU_MIRROR}|g" \
        /etc/apt/sources.list; \
    fi

# Drop any BuildKit-injected proxy: this image has no CA bundle yet.
RUN unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY; \
    apt-get update && apt-get install -y --no-install-recommends \
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

# Proxy only after CA certs exist. Mirrors stay in NO_PROXY.
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    GIT_SSL_CAINFO=/etc/ssl/certs/ca-certificates.crt \
    http_proxy=${http_proxy} \
    https_proxy=${https_proxy} \
    HTTP_PROXY=${HTTP_PROXY} \
    HTTPS_PROXY=${HTTPS_PROXY} \
    no_proxy=${no_proxy} \
    NO_PROXY=${NO_PROXY}

# GitHub git deps (async-openai, nucleo) and release downloads go through GH_PROXY.
RUN if [ -n "${GH_PROXY}" ]; then \
      git config --global url."${GH_PROXY}https://github.com/".insteadOf "https://github.com/"; \
    fi

# Match bin/protoc (DotSlash) so generated protobuf code stays in-range.
ARG PROTOC_VERSION=29.3
RUN curl -fsSL -o /tmp/protoc.zip \
        "${GH_PROXY}https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip" \
    && unzip -q /tmp/protoc.zip -d /usr/local \
    && rm /tmp/protoc.zip \
    && protoc --version

# rust-toolchain.toml pins 1.94.0; install that toolchain so cargo does not
# re-download it on every build. Installer + dist come from rsproxy.
ARG RUST_VERSION=1.94.0
RUN curl --proto '=https' --tlsv1.2 -sSf "${RUSTUP_INIT_URL}" | \
        sh -s -- -y --default-toolchain "${RUST_VERSION}" --profile minimal \
    && rustc --version

COPY docker/cargo-config.toml /usr/local/cargo/config.toml

WORKDIR /src
COPY . .

RUN --mount=type=cache,id=grok-cargo-registry-cn,target=/usr/local/cargo/registry \
    --mount=type=cache,id=grok-cargo-git-cn,target=/usr/local/cargo/git \
    --mount=type=cache,id=grok-cargo-target,target=/src/target \
    cargo build -p xai-grok-pager-bin --release \
    && cp /src/target/release/xai-grok-pager /tmp/grok \
    && strip /tmp/grok

FROM builder AS tester
RUN --mount=type=cache,id=grok-cargo-registry-cn,target=/usr/local/cargo/registry \
    --mount=type=cache,id=grok-cargo-git-cn,target=/usr/local/cargo/git \
    --mount=type=cache,id=grok-cargo-target,target=/src/target \
    unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY \
    && cargo test --locked -p xai-grok-http --lib \
    && cargo test --locked -p xai-grok-update --lib \
    && cargo test --locked -p xai-grok-shell --lib \
    && cargo test --locked -p xai-grok-pager-bin --bins

FROM scratch AS export
COPY --from=builder /tmp/grok /grok
