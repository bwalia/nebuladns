# ADR 0001 — Toolchain, async runtime, and HTTP framework

- **Status**: Accepted
- **Date**: 2026-04-17
- **Deciders**: @nebuladns/maintainers
- **Supersedes**: —

## Context

Per `PROJECT_PROMPT.md` §10 and §11, NebulaDNS must be a memory-safe, low-tail-latency
authoritative DNS server with a control plane exposed over HTTP and gRPC. We need to
commit to a toolchain and a set of foundational dependencies before wiring M0 subsystems
together.

## Decision

- **Language**: Rust, stable channel, MSRV 1.80. Pinned via `rust-toolchain.toml`.
- **Safety**: every crate begins with `#![forbid(unsafe_code)]`. Exceptions require a
  documented justification in the crate's README and a fuzz harness.
- **Async runtime**: `tokio` 1.40 with the `full` feature.
- **HTTP framework**: `axum` 0.7 for REST, `tonic` 0.12 for gRPC. They share `tower`
  middleware, so a single set of tracing / timeout / compression layers covers both.
- **OpenAPI**: generated from code via `utoipa` derives. Hand-maintained specs drift; a
  derived spec is always in sync with the types the server actually returns. This is the
  mechanical enforcement of the "API-first" principle.
- **Metrics**: `prometheus-client` (the official OpenMetrics Rust crate) — no locks, no
  boxing, const-generic labels. Always-on exposition on a separate bind.
- **Tracing**: `tracing` + `tracing-subscriber` with JSON layer; OTLP via
  `tracing-opentelemetry` once the collector ships.
- **TLS**: `rustls` 0.23. Ring / aws-lc-rs as the crypto backend is a later ADR.
- **Error types**: `thiserror` in library crates, `anyhow` in binaries.
- **CLI parsing**: `clap` 4.5 derive API.
- **Storage** (path-deferred): embedded backend decided in a later ADR; `sled` and
  `redb` are both candidates. `nebula-store` ships as an abstraction first.
- **Consensus** (path-deferred): Raft via `openraft` vs `raft-rs` is a later ADR
  (addressed in M7).
- **DNSSEC crypto** (path-deferred): `ring` vs `aws-lc-rs` is a later ADR (M4).

## Rationale

- `tokio` + `axum` is the default modern Rust stack and interoperates cleanly with
  `tonic`. It also gives us `io_uring` via `tokio-uring` once we flip the hot-path
  switch in M2.
- `prometheus-client` is the only Rust Prometheus crate that satisfies the "zero
  hot-path allocation" constraint in §6 of the prompt.
- `utoipa` integrates with `axum` via extractors, so we can derive the OpenAPI spec from
  the very handlers that serve the API. There is no opportunity for drift.
- `thiserror` gives us precise enum-typed errors (which the planning prompt calls for
  explicitly in §4 — "errors are explicit, not silently malformed").

## Consequences

- Every new dependency must clear `cargo deny` (`deny.toml` whitelists `Apache-2.0`,
  `MIT`, `BSD-*`, `ISC`, `Unicode-*`, `CC0-1.0`, `Zlib`, `MPL-2.0`).
- MSRV 1.80 means we can use `let … else`, `std::sync::LazyLock`, and named format args
  in `panic!` — but not newer features. Bumping MSRV requires an ADR.
- `rustls` means we compile without OpenSSL. This is intentional; OpenSSL's CVE surface
  is one of the risks we are specifically avoiding.

## Alternatives considered

- **`async-std` / `smol`**: smaller ecosystem, no `io_uring` story.
- **`hyper` directly, no `axum`**: rejected — `axum`'s extractor model makes `utoipa`
  integration trivial, and the overhead vs. raw `hyper` is negligible.
- **`warp`**: rejected — less active and no first-class OpenAPI story.
- **Go**: rejected — GC tail latency was an explicit failure of CoreDNS.
