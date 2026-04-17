# NebulaDNS architecture

This document is a one-page view of the target system. The authoritative source of
design intent is the planning prompt (preserved at `PROJECT_PROMPT.md`); this file
summarizes it so engineers can orient quickly without re-reading the full prompt.

## System diagram

```
                           ┌─────────────────────────────────────┐
                           │           React Web UI              │
                           │  (Vite + TS + Tanstack Query)       │
                           └───────────────┬─────────────────────┘
                                           │ HTTPS (OpenAPI)
                           ┌───────────────▼─────────────────────┐
                           │        Control Plane API            │
                           │  REST (axum) + gRPC (tonic)         │
                           │  AuthN: mTLS + OIDC   AuthZ: RBAC   │
                           └───────┬─────────────┬───────────────┘
                                   │             │
                    ┌──────────────▼───┐    ┌────▼─────────────┐
                    │  Zone Manager    │    │ Propagation      │
                    │  - validate      │    │ Verifier         │
                    │  - sign (DNSSEC) │    │ - polls secondaries
                    │  - version       │    │ - confirms SOA   │
                    │  - atomic commit │    │ - raises alerts  │
                    └──────────┬───────┘    └──────────────────┘
                               │
         ┌─────────────────────▼──────────────────────────┐
         │           Zone Store (content-addressed)       │
         │   sled / redb / pluggable backend              │
         └─────────────────────┬──────────────────────────┘
                               │
         ┌─────────────────────▼──────────────────────────┐
         │              DNS Data Plane                    │
         │   tokio + io_uring where available             │
         │   UDP/TCP/DoT/DoH/DoQ listeners                │
         │   AXFR/IXFR/NOTIFY server                      │
         │   DNSSEC signer (online)                       │
         └────────────────────────────────────────────────┘

  Observability spine: tracing → OpenTelemetry → Prometheus + Loki + Tempo
```

## Crate layout

| Crate | Purpose |
|---|---|
| `nebula-wire` | RFC 1035 wire codec, `#![forbid(unsafe_code)]`, fuzz-tested |
| `nebula-proto` | High-level DNS message types (no I/O) |
| `nebula-zone` | Zone parser, validator, serializer |
| `nebula-signer` | DNSSEC online signing + key management |
| `nebula-store` | Storage abstraction + embedded backends |
| `nebula-transfer` | AXFR / IXFR / NOTIFY / TSIG |
| `nebula-server` | DNS data plane (tokio) + `nebuladns` binary |
| `nebula-api` | REST (axum) + gRPC (tonic) + OpenAPI |
| `nebula-verify` | Propagation verifier |
| `nebula-metrics` | Always-on Prometheus registry |
| `nebula-cli` | `nebulactl` admin CLI |
| `nebula-testutil` | Test harness + reference-peer drivers |

## Why another authoritative DNS server

The incident history (`tinydns-cross-incident-analysis.md`) and the competitive table in
`PROJECT_PROMPT.md` §2.5 capture the full rationale. The short version:

- **djbdns 1.05** emits AXFR responses with `QDCOUNT=0`. BIND 9.18 rejects them. This
  silently eroded zone-transfer redundancy for 13 months until it caused two P0/P1
  production incidents.
- **BIND** has a 40-year CVE history, a file-on-disk config model, and no native
  propagation gate.
- **PowerDNS**'s API is incomplete (many operations require backend SQL edits).
- **Knot**, **NSD** have no REST API and no UI.
- **CoreDNS** has GC-induced tail-latency and is not a full authoritative server for
  public zones.

**No alternative** ships a first-class propagation gate (the feature that would have
prevented both incidents) or a Kubernetes operator with CRDs.

## Non-negotiable design principles

(Full detail in `PROJECT_PROMPT.md` §1.)

1. API-first — every operator action is a single authenticated API call.
2. Reliability-first — correctness > feature breadth.
3. Ultra-low latency — p50 ≤ 40 µs, p99 ≤ 150 µs.
4. Standards-conformant on the wire.
5. Observable by default — `/metrics` is always on, zero hot-path cost.
6. Verified propagation — a deploy isn't done until every required secondary ACKs.
7. Atomic, versioned configuration.
8. Deterministic SOA serial management.
9. Redundancy you can see.
10. Safety over cleverness (`#![forbid(unsafe_code)]`).
11. Cloud-native & Kubernetes-native.
12. Batteries included.

## M1 scope (current)

Delivered on top of M0:

- **Wire codec**: full RFC 1035 Message (header + question + all sections), name
  compression read + write with cycle detection, RR codec covering A/AAAA/NS/CNAME/SOA/
  MX/TXT/PTR/SRV/CAA, EDNS(0) OPT, unknown-type pass-through. 4 fuzz targets.
- **Zone loader** (`nebula-zone`): TOML schema, in-memory hash-indexed RRset store with
  case-insensitive lookup.
- **DNS data plane** (`nebula-server/src/dns.rs`): UDP and TCP listeners, tokio-based,
  `arc-swap`-backed zone registry, authoritative answer path with NXDOMAIN+SOA, NODATA,
  REFUSED for out-of-zone, UDP truncation (`TC=1`) on overflow.
- **Data-plane metrics**: `nebula_dns_queries_total`, `nebula_dns_query_duration_seconds`,
  `nebula_dns_dropped_total`, all with bounded enum-typed labels (`proto`/`qtype`/`rcode`
  capped at compile time).
- **RFC 4035 compliance fix**: header codec now recognizes the AD/CD DNSSEC bits
  (previously rejected as reserved Z bits — which caused `dig` queries to fail FORMERR
  in the first smoke test).

End-to-end validated with real `dig` over UDP and TCP.

## M0 scope (previous)

The skeleton that all subsequent milestones build on:

- Cargo workspace + CI pipeline.
- `/livez`, `/readyz`, `/metrics` served on separate binds.
- `nebuladns` binary with JSON tracing, config load, systemd `sd_notify` READY + watchdog,
  graceful shutdown on SIGINT/SIGTERM.
- `nebulactl` CLI with `version` and `health` subcommands.
- `deploy/systemd/nebuladns.service` (hardened, with watchdog).
- `deploy/docker/Dockerfile` (distroless, non-root, multi-stage) + `docker-compose.yml`.
- `deploy/helm/nebuladns/` chart (StatefulSet, Service, PDB, NetworkPolicy,
  ServiceMonitor, PrometheusRule).

## M1 → GA

See `PROJECT_PROMPT.md` §15 for the full milestone schedule.
