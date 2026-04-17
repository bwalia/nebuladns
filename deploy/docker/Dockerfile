# syntax=docker/dockerfile:1.7
#
# Multi-stage build for NebulaDNS.
#
# Stage 1 (builder): compiles the nebuladns + nebulactl binaries with a static musl target.
# Stage 2 (runtime): distroless + non-root. Final image ~25 MB compressed.

ARG RUST_VERSION=1.80
# TARGETPLATFORM is set automatically by buildx (e.g. `linux/amd64`, `linux/arm64`).
# We translate it into a Rust musl triple below so the runtime stage gets a static binary
# matching the platform it runs on. Cross-compiling to a different arch is the CI path
# (via `cross`); local `docker build` on Apple Silicon just builds aarch64-musl.

FROM rust:${RUST_VERSION}-bookworm AS builder
ARG TARGETPLATFORM
ENV CARGO_TERM_COLOR=always

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       musl-tools musl-dev pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN case "${TARGETPLATFORM}" in \
        "linux/amd64"|"") echo x86_64-unknown-linux-musl ;; \
        "linux/arm64")    echo aarch64-unknown-linux-musl ;; \
        *) echo "unsupported TARGETPLATFORM: ${TARGETPLATFORM}" >&2; exit 1 ;; \
    esac > /tmp/target.env

WORKDIR /build

# Cache dependency compilation by copying manifests first.
COPY Cargo.toml Cargo.lock* rust-toolchain.toml rustfmt.toml clippy.toml ./
COPY crates/ crates/
COPY xtask/ xtask/

# rustup target add + cargo build in one RUN so the target toolchain installed into
# $RUSTUP_HOME is visible to the build step in this layer. (Cache mounts don't shadow
# $RUSTUP_HOME, but splitting across RUNs has been flaky with some buildx versions.)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    TARGET="$(cat /tmp/target.env)" \
    && rustup target add "${TARGET}" \
    && cargo build --release --target "${TARGET}" \
           --bin nebuladns --bin nebulactl \
    && cp "target/${TARGET}/release/nebuladns"  /out-nebuladns \
    && cp "target/${TARGET}/release/nebulactl"  /out-nebulactl

# ---------------------------------------------------------------------------
FROM gcr.io/distroless/static-debian12:nonroot AS runtime
COPY --from=builder /out-nebuladns  /usr/bin/nebuladns
COPY --from=builder /out-nebulactl  /usr/bin/nebulactl
COPY config/nebuladns.example.toml /etc/nebuladns/nebuladns.toml

# Distroless "nonroot" = UID/GID 65532.
USER 65532:65532

# Admin HTTP, metrics, DNS (container maps to these for testing).
EXPOSE 53/udp 53/tcp 853/tcp 443/tcp 8080/tcp 9090/tcp

HEALTHCHECK --interval=10s --timeout=2s --start-period=5s --retries=3 \
    CMD ["/usr/bin/nebuladns", "--healthcheck"]

ENTRYPOINT ["/usr/bin/nebuladns"]
CMD ["--config", "/etc/nebuladns/nebuladns.toml"]

# OCI labels
LABEL org.opencontainers.image.title="NebulaDNS" \
      org.opencontainers.image.description="Modern authoritative DNS server in Rust" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/nebuladns/nebuladns" \
      org.opencontainers.image.url="https://nebuladns.io" \
      org.opencontainers.image.vendor="NebulaDNS Authors"
