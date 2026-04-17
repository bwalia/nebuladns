# Contributing to NebulaDNS

Thank you for considering a contribution. This document describes the process.

## Ground rules

1. **Every feature ships with a test, a metric, and a doc section.** No exceptions.
2. **API-first**: every operator-visible behavior must be expressible as an API call before
   any CLI or UI work is merged.
3. **No `unsafe` on the hot path** unless justified with a SAFETY comment and a fuzz harness.
   Prefer `#![forbid(unsafe_code)]`.
4. **Every Prometheus alert has a runbook** in `docs/runbooks/`. CI enforces this.
5. **Every non-obvious design choice needs an ADR** in `docs/decisions/`.

## Development setup

```bash
# Prerequisites: Rust stable (rust-toolchain.toml pins the channel)
rustup show
cargo --version

# Build + test
cargo build --workspace
cargo nextest run --workspace   # or: cargo test --workspace

# Lint
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

The `xtask` crate provides shortcuts:

```bash
cargo xtask ci         # full CI suite locally
cargo xtask fuzz wire  # smoke fuzz the wire codec
cargo xtask bench      # run benchmarks
```

## Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/). Examples:

- `feat(wire): add OPT pseudo-RR encoder`
- `fix(transfer): send AXFR with QDCOUNT=1`
- `docs(runbooks): add runbook for PropagationGateExceededSLA`
- `chore(deps): bump rustls to 0.23`

## Pull requests

- One logical change per PR
- Link the issue in the description
- CI must be green
- A code owner review is required

## Reporting security issues

See [`SECURITY.md`](SECURITY.md). Do not open a public issue.

## Code of conduct

All contributors are expected to follow the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
