# NebulaDNS — Project Planning Prompt

**Intended audience:** an AI coding assistant (or an engineering team) that will build a next-generation authoritative DNS server in Rust.
**Date:** 2026-04-17
**Author:** Balinder Walia
**Status:** Planning / pre-implementation

---

## 0. Why this project exists (read this first)

We run TinyDNS (djbdns 1.05, released 2001) as the hidden master behind Akamai. In the last 22 days we had **two production outages** on `ticketmaster.pl` and `ticketmaster.com` (P1 and P0 respectively). The cross-incident analysis (`tinydns-cross-incident-analysis.md`) traced both to:

1. **djbdns 1.05's axfrdns sends AXFR responses with `QDCOUNT=0`**, which BIND 9.18+ rejects as FORMERR per RFC 1034 §4.3.3. Akamai's Zone Transfer Agents silently upgraded to BIND 9.18, so our zone-transfer redundancy **eroded invisibly for 13+ months** until a single agent remained.
2. **No post-deploy verification.** The pipeline reported "deploy complete" while downstream secondaries were serving stale data. End users, not the pipeline, detected the outage.
3. **Opaque operational surface.** No `/metrics`, no API, no dashboard. Root cause required packet captures and forum archaeology. Engineers had to SSH into six servers to reconstruct state.
4. **SOA serial handling was fragile.** Retries with the same serial wedged recovery; recovery required manual serial bumps.
5. **Change propagation blindness.** There was no signal that Akamai had failed to consume a zone; internal users also suffered 30–60 minutes of extended impact from Infoblox negative-cache TTLs.

**We are not going to patch a 25-year-old codebase again.** We are building a replacement: a modern, observable, safe-by-default authoritative DNS server in Rust, with a first-class control plane — API, metrics, and a web UI — so the next failure is detected in seconds, not by customers.

Call it **NebulaDNS**.

---

## 1. Non-negotiable design principles

These principles directly answer failure modes from incidents 1273 and 1326 and the limitations of existing DNS software (see §2.5). Every design decision below should be traceable back to one of them.

1. **API-first.** Every operator action (create zone, edit record, rotate TSIG key, trigger rollover, force NOTIFY, query health, rollback deploy) MUST be expressible as a single authenticated API call. The CLI, React UI, Terraform provider, Kubernetes operator, and CI/CD integrations are all thin clients over the same OpenAPI-described REST + gRPC surface. **If you cannot do it via the API, it does not ship.** There are no "edit this file and SIGHUP" workflows; there are no hidden admin shells. This is the core differentiator vs. BIND, Knot, NSD, and TinyDNS, where config-file-on-disk is the primary interface.
2. **Reliability-first.** Correctness, safety, and recoverability come before feature breadth. Every write path is transactional with rollback; every deploy is gated by propagation verification; every hot path is fuzz-tested; every failure mode is observable. The server MUST fail closed (serve stale-but-valid > serve wrong), MUST never lose a zone version, and MUST surface degraded state before customers do. SLO budget for unplanned downtime on the data plane: ≤ 5 minutes/year per node (five-nines).
3. **Ultra-low latency, very high performance.** Hot-path design uses `io_uring` (with epoll fallback), a lock-free zone store backed by a persistent radix/qp-trie, zero-copy wire encode/decode, pinned worker threads, and huge pages where available. Targets (see §11 for full detail): **p50 ≤ 40 µs, p99 ≤ 150 µs in-process**, **≥ 2 Mqps/node** on modern 16-core x86_64 for UDP simple answers. Every allocation on the hot path is scrutinized; `alloc`-free steady state is the goal.
4. **Standards-conformant on the wire.** Strict adherence to RFC 1034/1035, RFC 5936 (AXFR), RFC 1995 (IXFR), RFC 1996 (NOTIFY), RFC 2136 (Dynamic Update), RFC 2845/8945 (TSIG), RFC 4034/4035 (DNSSEC), RFC 7766 (DNS over TCP), RFC 7858 (DoT), RFC 8484 (DoH), RFC 9250 (DoQ). **No silent protocol shortcuts.** Add a conformance test suite that runs against BIND, Knot, PowerDNS, NSD, CoreDNS, and djbdns as reference peers.
5. **Observable by default.** Every request, transfer, NOTIFY, and config reload emits structured logs and Prometheus metrics. Nothing important happens without a signal.
6. **Verified propagation.** A deploy is not "done" until the server has confirmed that every declared downstream secondary has acknowledged the new SOA serial. No more "deploy complete" lies.
7. **Atomic, versioned configuration.** Zone data is content-addressed and deployed atomically. No partial reads, no mid-write polls. Roll forward and rollback are one-liners.
8. **Deterministic SOA serial management.** Auto-generated, monotonic serials. Retry-with-same-serial is impossible by construction.
9. **Redundancy you can see.** The dashboard shows, at a glance, which secondaries last successfully transferred each zone, and when. No silent erosion.
10. **Safety over cleverness.** Written in safe Rust. Zero `unsafe` in the hot path unless justified with a SAFETY comment and a fuzz harness. Use `#![forbid(unsafe_code)]` per crate where possible.
11. **Cloud-native & Kubernetes-native.** Ships as a container image, a Helm chart, and a Kubernetes operator with CRDs (`Zone`, `Record`, `Secondary`, `TsigKey`, `DeployGate`). Works as a drop-in cluster DNS replacement for CoreDNS, integrates with ExternalDNS as a provider, and supports GitOps (Flux/ArgoCD) out of the box.
12. **Batteries included.** `/metrics`, `/livez`, `/readyz`, OpenAPI/Swagger, React dashboard, CLI, systemd unit, Debian/RPM installer, Docker image, Helm chart, Kubernetes operator, and Terraform provider ship in-tree. Operators should not have to assemble their own tooling.

---

## 2. Scope

### In scope (v1.0)

- Authoritative DNS server (primary + secondary roles, single binary).
- Protocols: UDP/53, TCP/53, DoT/853, DoH/443, DoQ/853 (QUIC).
- Zone transfer: AXFR (RFC 5936), IXFR (RFC 1995), NOTIFY (RFC 1996), TSIG (RFC 8945).
- Dynamic updates (RFC 2136) with ACL + TSIG.
- DNSSEC online signing (RSA, ECDSA P-256, Ed25519) with automatic key rollover (RFC 6781).
- Views / geo-routing / weighted round-robin / latency-based answers.
- Response Policy Zones (RPZ) for threat feeds.
- Zone data formats: RFC 1035 master files, YAML/JSON, and a native TOML schema.
- Configuration: declarative, hot-reloadable, validated before apply.
- Control plane: HTTPS REST API + gRPC + Admin CLI + React UI.
- Metrics: Prometheus `/metrics`, OpenTelemetry traces, structured JSON logs.
- Storage backends: embedded (sled/redb) for single-node; pluggable (Postgres, FoundationDB) for clusters.
- Pluggable anycast via BGP speaker integration (bird/gobgp sidecar, or in-process via `rustybgp`/`holo`).
- First-class integration tests against BIND 9.18, Knot 3.x, PowerDNS 4.x, NSD 4.x, CoreDNS 1.11 as peers.
- **Installers & packaging (see §13 for full matrix):** systemd unit, `.deb` and `.rpm` packages, Homebrew formula, FreeBSD port, `install.sh` one-liner.
- **Container image** (multi-arch, distroless, SBOM-attested, signed with cosign).
- **Helm chart** (single-node, multi-node, cluster-DNS replacement modes).
- **Kubernetes operator with CRDs** — `Zone`, `Record`, `Secondary`, `TsigKey`, `DeployGate`, `Policy`.
- **ExternalDNS provider** — NebulaDNS as a first-class ExternalDNS backend.
- **CoreDNS compatibility** — drop-in for Kubernetes cluster DNS with `Corefile` translation on import.

### Out of scope (v1.0, revisit later)

- Recursive resolver mode (separate product; see v2 roadmap).
- Managed-DNS multi-tenant SaaS (the server supports it, we won't ship the SaaS layer in v1).
- GUI-based zone editing for non-technical users (API + CLI + declarative config first).
- Windows Server support (Linux/macOS/FreeBSD only; Windows development containers are fine).

### 2.5. Competitive analysis — what's wrong with the alternatives

We evaluated every serious authoritative DNS server in production use today. Each has one or more structural limitations that NebulaDNS is specifically designed to overcome. This table is the single most important justification for building new software rather than adopting an existing one.

| DNS Software | Strengths | Critical Limitations | NebulaDNS Answer |
|---|---|---|---|
| **TinyDNS / djbdns 1.05** | Tiny, fast, stable code (too stable — frozen since 2001) | No DNSSEC, no IXFR, no NOTIFY auth, no TSIG in original, `QDCOUNT=0` wire bug (caused our incidents), no metrics, no API, no reload without restart, C code without modern safety, unmaintained, no IPv6 in classic build | Full RFC-conformant wire, modern Rust safety, API-first, `/metrics` first-class, atomic hot reload, TSIG/DNSSEC built in |
| **BIND 9** | Reference implementation, broadest feature set, widely deployed | Monolithic C codebase with long CVE history (dozens per year), config (`named.conf`) is file-only and fragile, `rndc` is limited, `statistics-channels` is XML/JSON without Prometheus, no built-in UI, zone-transfer errors surface as opaque log lines, DNSSEC operations require separate `dnssec-*` tools, 40 years of accumulated cruft, non-trivial memory footprint | Single small binary, memory-safe Rust, `/metrics` native, REST + gRPC API for every operation, integrated UI, unified DNSSEC management via API, structured logs and traces |
| **PowerDNS Authoritative** | SQL/LDAP backends, has a REST API, good DNSSEC support | **API is incomplete** (many operations still require backend SQL edits), multiple moving parts (authoritative + dnsdist + recursor + Postgres), the API does not cover propagation verification, no native propagation gate, requires external monitoring, dashboards are third-party and inconsistent, performance ceiling constrained by SQL backend on hot path | API covers 100% of operations (API-first principle), propagation verifier is built-in, single binary (no separate recursor/dist), embedded store with pluggable SQL as optional backend, ships with dashboards |
| **Knot DNS** | Very fast (C, lock-free), clean codebase, good DNSSEC | No native REST API (control is via `knotc` CLI + YAML), no built-in web UI, no propagation verification, metrics are via module but limited, operator tooling is DIY | REST + gRPC + CLI + UI from day one, propagation verifier native, richer metrics schema |
| **NSD (NLnet Labs)** | Minimal, very fast, secure-by-design | Intentionally minimal: no dynamic updates, no online DNSSEC signing (offline only), no API, no UI, no geo-routing, no views, control is `nsd-control` only | Full-featured while retaining NSD-class performance via Rust and modern kernel I/O |
| **CoreDNS** | Plugin architecture, Kubernetes-native, Go | **Not a full-fledged authoritative server for internet-facing zones** — no AXFR/IXFR server (only client in `secondary` plugin — limited), DNSSEC is primitive (no online signing in standard build), no built-in UI, plugin compatibility burden, Go GC tail latency at high QPS | First-class authoritative + transfer + DNSSEC + UI; drop-in replacement for CoreDNS as cluster DNS while also serving public zones; consistent low-tail-latency (no GC) |
| **Unbound** (resolver; listed for completeness) | Excellent resolver | Not authoritative (out of our lane) | N/A — we are explicitly authoritative-only in v1 |
| **Route 53 / Cloudflare DNS** | Managed, anycast, great UX | Vendor lock-in, cost at scale, no self-hosted option, limited visibility into internals, no on-prem story | Self-hosted with the same UX quality; optionally integrates with managed providers as secondaries |
| **dnsdist** (load balancer; listed for completeness) | Great LB for DNS | Not authoritative | Built-in anycast + in-process LB policies remove the need for a separate dnsdist tier in many topologies |

**The common pattern across every open-source alternative:** the control plane is an afterthought bolted onto a 1990s/2000s-era data plane. NebulaDNS inverts this — the API is the server, and the data plane is a first-class implementation of the API's contract.

**Specific limitations NebulaDNS MUST overcome, spelled out:**

1. **No partial API surface.** Every BIND `rndc` verb, every PowerDNS API endpoint, every `knotc` command — we commit to a superset, and everything the CLI/UI does is an API call.
2. **No file-on-disk as source of truth.** Zones live in a content-addressed store, not in `db.example.com` files. The file is an export format, not the source of truth. (TinyDNS's `data.cdb` was our single point of failure during incident 1273 — never again.)
3. **No opaque zone-transfer failures.** BIND logs `zone transfer failed: FORMERR` with no peer version or packet detail. We record the full peer software fingerprint, packet capture (optional), and surface it in the UI with a direct link to a runbook.
4. **No garbage-collected tail latency.** CoreDNS's Go GC produces visible p99.9 hiccups at 100k+ qps. Rust's deterministic allocation eliminates this class of problem.
5. **No offline-only DNSSEC.** NSD requires an external signer. BIND's `dnssec-signzone` is a separate tool. We sign inline via a memory-safe signer with HSM/KMS support.
6. **No GitOps blindness.** None of the alternatives ship a Kubernetes operator with CRDs. We do — `Zone`, `Record`, `Secondary`, `TsigKey` — and they reconcile into the server state via the same API.
7. **No propagation gate.** This is the single most load-bearing missing feature across the entire ecosystem. None of BIND, Knot, NSD, PowerDNS, CoreDNS ships a first-class primitive that says "this change has reached all required downstreams, OK to proceed." NebulaDNS does. See §7.
8. **No UI worth using.** BIND has none. PowerDNS-Admin is a third-party Flask app. Knot has nothing. We ship a React dashboard whose top-priority feature is making incident 1326 visible in seconds.

---

## 3. Architecture

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

### Crate layout (Cargo workspace)

```
nebuladns/
├── crates/
│   ├── nebula-wire/          # RFC 1035 wire codec, fuzz-tested
│   ├── nebula-proto/         # High-level message types, no I/O
│   ├── nebula-zone/          # Zone parser, validator, serializer
│   ├── nebula-signer/        # DNSSEC signing & key management
│   ├── nebula-store/         # Storage abstraction + sled/redb impls
│   ├── nebula-transfer/      # AXFR/IXFR/NOTIFY/TSIG
│   ├── nebula-server/        # DNS data plane (tokio)
│   ├── nebula-api/           # axum REST + OpenAPI + tonic gRPC
│   ├── nebula-verify/        # Propagation verifier
│   ├── nebula-metrics/       # Prometheus registry + OTel bridge
│   ├── nebula-cli/           # `nebulactl` admin CLI
│   └── nebula-testutil/      # Test harness + reference-peer drivers
├── ui/                       # React dashboard (Vite + TS)
├── docs/
├── deploy/                   # Helm, systemd, Docker, Terraform module
└── xtask/                    # Dev-loop tasks (fmt, lint, fuzz, bench)
```

---

## 4. Data plane details

### Wire codec (`nebula-wire`)

- Hand-rolled zero-copy parser/encoder. No `unsafe`.
- **Strict parsing**: RFC 1035 compliance is enforced, but errors are explicit (`ParseError::QdCountMismatch { expected, got }`) so operators can reason about incompatibilities — **not** silently malformed like djbdns's QDCOUNT=0.
- Continuous fuzzing via `cargo fuzz` (libFuzzer + AFL++). Corpus seeded from real-world captures.
- Property tests via `proptest` — roundtrip `encode(decode(x)) == x` for all valid messages.

### Transport listeners

- **UDP/TCP**: tokio-based; per-listener concurrency and rate limits.
- **DoT (RFC 7858)**: rustls with ring or aws-lc-rs as crypto backend.
- **DoH (RFC 8484)**: axum on the same port family as the control plane but on a distinct bind (never share a listener with admin endpoints).
- **DoQ (RFC 9250)**: quinn-based, behind a feature flag for v1.0.
- EDNS0 (RFC 6891) mandatory, including cookies (RFC 7873) for DDoS mitigation.

### Zone transfer (`nebula-transfer`)

- **AXFR** (RFC 5936): streams records; supports multi-message responses, correct QDCOUNT=1, correct SOA-first-and-last framing.
- **IXFR** (RFC 1995): journal-backed; falls back to AXFR on journal miss.
- **NOTIFY** (RFC 1996): primary notifies all declared secondaries immediately on SOA bump. Retries with exponential backoff.
- **TSIG** (RFC 8945): HMAC-SHA256+; constant-time comparison; key rotation API.
- **Interop matrix** (CI): transfer tests against BIND 9.18, BIND 9.20, Knot 3.x, PowerDNS 4.x, NSD 4.x, Akamai Fast DNS (if we can get a staging lane). **This is the test that would have caught our 1326 bug on day one.**

### DNSSEC (`nebula-signer`)

- Online signing with NSEC or NSEC3 (operator-selected per zone).
- Algorithms: RSASHA256, ECDSAP256SHA256, ED25519.
- Automatic KSK/ZSK rollover with pre-publish and double-signature schemes (RFC 6781).
- Key storage: encrypted at rest; optional HSM (PKCS#11) and AWS KMS backends.

---

## 5. Control plane

### REST API (axum + utoipa for OpenAPI)

- Base path: `/api/v1`.
- OpenAPI 3.1 spec generated from `utoipa` derives — **always in sync with code**.
- **Swagger UI** served at `/api/docs` (production-safe: gated behind auth, read-only in prod).
- **ReDoc** alternative at `/api/redoc`.
- Every endpoint emits trace spans and request metrics.

#### Endpoint catalogue (non-exhaustive)

| Method | Path                                      | Purpose                                            |
|--------|-------------------------------------------|----------------------------------------------------|
| GET    | `/api/v1/zones`                           | List zones (paginated, filterable)                 |
| POST   | `/api/v1/zones`                           | Create zone (dry-run supported)                    |
| GET    | `/api/v1/zones/{zone}`                    | Zone metadata + current SOA                        |
| PUT    | `/api/v1/zones/{zone}`                    | Replace zone (atomic, versioned)                   |
| POST   | `/api/v1/zones/{zone}/records`            | Add/modify records (batch, transactional)          |
| GET    | `/api/v1/zones/{zone}/history`            | Version history with diff                          |
| POST   | `/api/v1/zones/{zone}/rollback`           | Rollback to previous version                       |
| GET    | `/api/v1/zones/{zone}/propagation`        | Per-secondary propagation state (see §7)           |
| POST   | `/api/v1/zones/{zone}/notify`             | Force NOTIFY to all secondaries                    |
| GET    | `/api/v1/zones/{zone}/dnssec`             | DNSSEC status + keys                               |
| POST   | `/api/v1/zones/{zone}/dnssec/rollover`    | Trigger key rollover                               |
| GET    | `/api/v1/secondaries`                     | List declared secondaries + health                 |
| GET    | `/api/v1/secondaries/{id}/health`         | Per-secondary transfer history, FORMERR count, RTT |
| GET    | `/api/v1/keys/tsig`                       | List TSIG keys (metadata only)                     |
| POST   | `/api/v1/keys/tsig`                       | Generate TSIG key                                  |
| POST   | `/api/v1/keys/tsig/{name}/rotate`         | Rotate TSIG key                                    |
| POST   | `/api/v1/deploy`                          | Deploy pending changes with propagation gate       |
| GET    | `/api/v1/deploys/{id}`                    | Deploy status (pending, verifying, verified, failed) |
| GET    | `/api/v1/events`                          | SSE stream of server events                        |

Every write operation is **idempotent** (idempotency keys in headers), **dry-run-able** (`?dry_run=true`), and **audit-logged**.

### gRPC API (tonic)

- Mirrors the REST API for automation/pipeline integration.
- Bidirectional streaming for `watchDeploys` and `watchPropagation`.

### Admin CLI (`nebulactl`)

- `nebulactl zone list / show / apply / diff / rollback / notify`
- `nebulactl deploy --wait-for-propagation --timeout 60s`
- `nebulactl secondaries health` — shows the exact view incident 1326 lacked
- `nebulactl keys tsig rotate <name>`
- Outputs: human (default), JSON, YAML, table.

### Authentication & authorization

- **AuthN**: mTLS (primary for machines), OIDC (for humans via SSO), static API tokens (break-glass).
- **AuthZ**: RBAC with roles `viewer`, `editor`, `approver`, `admin`, plus per-zone scoping.
- All admin actions signed into an append-only audit log (hash-chained).

---

## 6. Observability

### Metrics (`/metrics`, Prometheus text format) — MUST be always-on with zero hot-path cost

Metrics are **always on in production**. There is no "enable metrics" flag. An operator should never be in the position of choosing between observability and performance — because if that choice ever exists, the choice made under incident pressure will be the wrong one. Incident 1326 burned us partly because there was no `/metrics` endpoint; this is how we permanently retire that class of failure.

#### Design contract (non-negotiable)

1. **Prometheus text exposition format** (also OpenMetrics on negotiated `Accept` header) on `/metrics`, default `127.0.0.1:9090` (configurable). Serve gzip if requested.
2. **Zero hot-path allocation.** Every counter/gauge/histogram is pre-registered at startup; hot-path code does only relaxed atomic add/load. No locks, no hashmap lookups, no label-value formatting per request.
3. **Label cardinality is bounded at compile time.** High-cardinality fields (client IP, qname, RR owner) are NEVER metric labels — they go to logs and traces. Enforced by a proc-macro that rejects unbounded labels; CI also runs a cardinality-budget check against a live instance (`nebula-metrics-lint`).
4. **Histograms use native histograms** (sparse bucketing, Prometheus 2.40+) where the scraper supports it, with classic-histogram fallback. Buckets chosen for ≤ 1% bucket-relative error across the full operating range.
5. **Scrape cost is < 10 ms** for a 10k-zone deployment; `/metrics` rendering uses a pre-sized buffer and serves bytes directly.
6. **Performance budget**: full metrics ON must cost ≤ 1% of hot-path CPU and add ≤ 1 µs to p99 latency vs. a metrics-OFF build. This is measured in CI on every PR by a dedicated `bench/metrics-overhead/` harness; regressions block merge.
7. **Sampling, not toggling.** If metrics ever become a bottleneck under extreme load, response-time histograms automatically shift to **reservoir sampling** (configurable `HDRHistogram` fast-path + periodic merge). Counters are never sampled.
8. **All metrics self-describe.** `# HELP` and `# TYPE` on every metric, units in the name (`_seconds`, `_bytes`, `_total`), `_info` metrics for build/version.
9. **No duplicate series across scrapes.** Metric registry is static for the process lifetime.
10. **Safe to enable on the data plane node itself.** Scrape endpoint is bound on a separate socket from DNS traffic, with its own rate limiter and a CPU cgroup allocation if systemd / Kubernetes is used.

#### Implementation choices

- **Library**: `prometheus-client` (the official OpenMetrics Rust crate) — const-generics, no locks, no boxing.
- **Storage**: counters and gauges use `AtomicU64` with `Ordering::Relaxed`; this is a single `lock xadd` on x86_64 — effectively free on the hot path.
- **Histograms**: lock-free striped histogram (per-CPU slot), summed at scrape time. Based on `hdrhistogram-rs` for latency-grade accuracy.
- **Labels**: pre-interned as `&'static str`; all known label combinations pre-materialized at startup (enum-typed labels ensure exhaustiveness).
- **Exposition path runs outside the hot path** on a dedicated scrape worker, so a slow scraper never backpressures query serving.

#### Required metric set (exhaustive for v1.0)

Every line below is a production metric with a defined owner, SLO, and alert. Grouped by pillar.

**Wire / query pipeline**

```
# HELP nebula_dns_queries_total Queries received, by transport and outcome.
# TYPE nebula_dns_queries_total counter
nebula_dns_queries_total{proto="udp|tcp|dot|doh|doq",qtype,rcode,zone,view}

# HELP nebula_dns_query_duration_seconds Time to produce a response (in-process).
# TYPE nebula_dns_query_duration_seconds histogram
nebula_dns_query_duration_seconds_bucket{proto,qtype,le="..."}  # native histogram
nebula_dns_query_duration_seconds_sum
nebula_dns_query_duration_seconds_count

# HELP nebula_dns_response_bytes Response size distribution.
# TYPE nebula_dns_response_bytes histogram
nebula_dns_response_bytes_bucket{proto,qtype,le}

# HELP nebula_dns_dropped_total Requests dropped before response (malformed, policy, overload).
# TYPE nebula_dns_dropped_total counter
nebula_dns_dropped_total{reason="malformed|rate_limited|policy|overload|tsig_fail|acl_deny"}

# HELP nebula_dns_truncated_total Responses truncated (UDP size limit).
# TYPE nebula_dns_truncated_total counter
nebula_dns_truncated_total{proto}

# HELP nebula_dns_formerr_total FORMERR responses produced or received — the 1273/1326 signal.
# TYPE nebula_dns_formerr_total counter
nebula_dns_formerr_total{peer,direction="sent|received",reason}

# HELP nebula_dns_edns_cookie_mismatch_total EDNS cookie validation failures.
# TYPE nebula_dns_edns_cookie_mismatch_total counter
nebula_dns_edns_cookie_mismatch_total{peer}

# HELP nebula_dns_rrl_rejected_total Responses suppressed by response-rate-limiting.
# TYPE nebula_dns_rrl_rejected_total counter
nebula_dns_rrl_rejected_total{policy}

# HELP nebula_dns_nxdomain_total NXDOMAIN responses, per zone.
# TYPE nebula_dns_nxdomain_total counter
nebula_dns_nxdomain_total{zone}

# HELP nebula_dns_servfail_total SERVFAIL responses — correctness signal.
# TYPE nebula_dns_servfail_total counter
nebula_dns_servfail_total{zone,reason}
```

**Zone transfer (AXFR/IXFR/NOTIFY)**

```
# HELP nebula_axfr_attempts_total Zone transfer attempts.
# TYPE nebula_axfr_attempts_total counter
nebula_axfr_attempts_total{peer,zone,direction="out|in",result="success|formerr|timeout|tsig_fail|refused|truncated|network"}

# HELP nebula_axfr_duration_seconds AXFR completion time.
# TYPE nebula_axfr_duration_seconds histogram
nebula_axfr_duration_seconds_bucket{peer,zone,direction,le}

# HELP nebula_axfr_bytes_total Bytes transferred in AXFR.
# TYPE nebula_axfr_bytes_total counter
nebula_axfr_bytes_total{peer,zone,direction}

# HELP nebula_axfr_last_success_timestamp Unix ts of last successful AXFR per (peer,zone).
# TYPE nebula_axfr_last_success_timestamp gauge
nebula_axfr_last_success_timestamp_seconds{peer,zone}

# HELP nebula_ixfr_attempts_total IXFR attempts.
# TYPE nebula_ixfr_attempts_total counter
nebula_ixfr_attempts_total{peer,zone,direction,result}

# HELP nebula_ixfr_fallback_to_axfr_total IXFR → AXFR fallbacks (journal miss, etc.).
# TYPE nebula_ixfr_fallback_to_axfr_total counter
nebula_ixfr_fallback_to_axfr_total{peer,zone,reason}

# HELP nebula_notify_sent_total NOTIFY messages sent to secondaries.
# TYPE nebula_notify_sent_total counter
nebula_notify_sent_total{peer,zone,result="ack|timeout|refused"}

# HELP nebula_notify_roundtrip_seconds NOTIFY RTT.
# TYPE nebula_notify_roundtrip_seconds histogram
nebula_notify_roundtrip_seconds_bucket{peer,le}

# HELP nebula_peer_version_info Detected peer software fingerprint.
# TYPE nebula_peer_version_info gauge
nebula_peer_version_info{peer,software="bind|knot|nsd|powerdns|unbound|tinydns|nebuladns|unknown",version} 1
```

**Propagation verification (the load-bearing pillar)**

```
# HELP nebula_zone_current_soa_serial Currently authoritative SOA serial.
# TYPE nebula_zone_current_soa_serial gauge
nebula_zone_current_soa_serial{zone}

# HELP nebula_secondary_observed_soa_serial Most recent SOA serial observed at a secondary.
# TYPE nebula_secondary_observed_soa_serial gauge
nebula_secondary_observed_soa_serial{zone,peer}

# HELP nebula_zone_propagation_lag_seconds Time since secondary last converged to current serial.
# TYPE nebula_zone_propagation_lag_seconds gauge
nebula_zone_propagation_lag_seconds{zone,peer}

# HELP nebula_zone_propagation_converged A gauge set to 1 once all required secondaries are on current serial.
# TYPE nebula_zone_propagation_converged gauge
nebula_zone_propagation_converged{zone} 0|1

# HELP nebula_propagation_verifier_poll_duration_seconds Time to poll all secondaries in a cycle.
# TYPE nebula_propagation_verifier_poll_duration_seconds histogram
nebula_propagation_verifier_poll_duration_seconds_bucket{le}
```

**DNSSEC**

```
nebula_dnssec_signatures_remaining{zone}                     # gauge: shortest RRSIG runway in seconds
nebula_dnssec_signing_duration_seconds_bucket{zone,algo,le}  # histogram
nebula_dnssec_keys_total{zone,role="ksk|zsk",algo}           # gauge
nebula_dnssec_key_age_seconds{zone,key_tag,role}             # gauge
nebula_dnssec_rollover_state{zone} 0|1|2|3                   # gauge: 0=idle,1=pre-pub,2=double-sig,3=revoke
nebula_dnssec_validation_failures_total{zone,reason}         # counter (when NebulaDNS is also a validator)
```

**Deploy / change management**

```
nebula_deploys_total{zone,result="accepted|rejected|rolled_back"}
nebula_deploy_duration_seconds_bucket{zone,phase="validate|commit|notify|verify",le}
nebula_deploy_verification_duration_seconds_bucket{zone,le}
nebula_deploy_pending_zones                 # gauge
nebula_deploy_in_flight                     # gauge
nebula_audit_log_entries_total{action}
```

**Control plane (API)**

```
nebula_api_requests_total{method,path,status}
nebula_api_request_duration_seconds_bucket{method,path,le}
nebula_api_auth_failures_total{scheme="mtls|oidc|token",reason}
nebula_api_rbac_denials_total{role,action}
nebula_api_idempotency_cache_hits_total
nebula_api_websocket_streams_open            # gauge
```

**HA / replication (for the multi-region topology in §14.5)**

```
nebula_raft_role{region,node} 0|1|2                              # 0=follower,1=candidate,2=leader
nebula_raft_term{region}
nebula_raft_last_applied_index{region}
nebula_raft_commit_index{region}
nebula_replication_lag_seconds{source_region,dest_region}
nebula_replication_changesets_shipped_total{source_region,dest_region,result}
nebula_replication_conflicts_total{zone,resolution}
nebula_region_role{region} 0|1|2                                 # 0=standby,1=primary,2=read-only
nebula_leader_elections_total{region,reason}
```

**Storage**

```
nebula_store_read_duration_seconds_bucket{op="get|scan",le}
nebula_store_write_duration_seconds_bucket{op,le}
nebula_store_entries{kind="zone|record|key"}                     # gauge
nebula_store_bytes_on_disk{kind}                                 # gauge
nebula_store_wal_lag_bytes                                       # gauge
nebula_store_compactions_total
```

**Runtime (always-on)**

```
nebula_build_info{version,commit,rustc,target} 1
nebula_process_start_time_seconds
nebula_process_open_fds
nebula_process_resident_memory_bytes
nebula_process_virtual_memory_bytes
nebula_process_cpu_seconds_total
nebula_tokio_workers
nebula_tokio_tasks_alive
nebula_tokio_poll_count_total
nebula_tokio_slow_polls_total{threshold="10ms|100ms"}
nebula_io_uring_submissions_total
nebula_io_uring_completions_total
nebula_memory_hot_path_allocations_total                         # MUST be 0 or near-zero in steady state
```

**Cost-observability meta-metrics** (observing observability)

```
nebula_metrics_series_count                                      # gauge: total series count (cardinality budget)
nebula_metrics_scrape_duration_seconds                           # gauge: last scrape render time
nebula_metrics_scrape_response_bytes                             # gauge
nebula_metrics_cardinality_budget_utilization                    # gauge: 0.0..1.0
```

#### Alerting rules (ship in-tree as `deploy/prometheus/alerts.yaml`)

The canonical set. Every rule has a runbook link. New alerts must include a runbook or CI fails.

| Alert | Condition | Severity | Incident it would have caught |
|---|---|---|---|
| `SecondaryNoTransferSince5m` | `time() - nebula_axfr_last_success_timestamp_seconds > 300` | page | 1273 & 1326 (13-month silent erosion) |
| `PropagationGateExceededSLA` | `nebula_zone_propagation_converged == 0 for 60s` after a deploy | page | 1273 & 1326 |
| `FormerrFromPeerRising` | `rate(nebula_dns_formerr_total{direction="received"}[5m]) > 0.1` | page | 1326 |
| `PeerVersionChanged` | `changes(nebula_peer_version_info[1h]) > 0` | ticket | would have flagged Akamai's BIND 9.18 rollout |
| `DNSSECSignatureExpiresSoon` | `nebula_dnssec_signatures_remaining < 86400` | page | DNSSEC correctness |
| `AxfrFailureRateHigh` | `rate(nebula_axfr_attempts_total{result!="success"}[5m]) > 0.01` | page | |
| `ReplicationLagHigh` | `nebula_replication_lag_seconds > 30` | page | HA correctness |
| `RaftLeadershipFlapping` | `rate(nebula_leader_elections_total[10m]) > 3` | page | HA stability |
| `NebulaDNSQueryLatencyP99High` | `histogram_quantile(0.99, rate(nebula_dns_query_duration_seconds_bucket[5m])) > 0.001` | ticket | Performance regression |
| `NebulaDNSCPUBurn` | cgroup CPU > budget for 10m | ticket | |
| `CardinalityBudgetExceeded` | `nebula_metrics_cardinality_budget_utilization > 0.9` | ticket | Observability hygiene |
| `HotPathAllocations` | `rate(nebula_memory_hot_path_allocations_total[5m]) > 0` | ticket | Performance regression |
| `ApiErrorRateHigh` | `rate(nebula_api_requests_total{status=~"5.."}[5m]) / rate(nebula_api_requests_total[5m]) > 0.01` | page | |
| `AuditLogChainBroken` | any verification failure | page | Security / compliance |

#### Grafana dashboards

Pre-built, ship in `deploy/grafana/` and as Helm `GrafanaDashboard` CRs:

1. **NebulaDNS Overview** — golden signals, per-zone tiles.
2. **NebulaDNS Data Plane** — QPS, latency histograms (native), RCODE mix, per-transport breakdown.
3. **NebulaDNS Transfers & Propagation** — per-peer matrix, SOA divergence, peer version fingerprints. **This is the dashboard that would have made incident 1326 impossible to miss.**
4. **NebulaDNS DNSSEC** — signature runway, key lifecycle, rollover timelines.
5. **NebulaDNS Control Plane** — API latency, error rates, audit-log activity.
6. **NebulaDNS HA & Replication** — Raft state, replication lag, region roles, conflict rates.
7. **NebulaDNS Runtime & Cost** — CPU, memory, io_uring throughput, cardinality utilization.

#### Exporters for peer observability

Because incident 1326 required fingerprinting Akamai's BIND version, NebulaDNS ships companion exporters in `integrations/exporters/`:

- `nebula-peer-exporter`: scrapes `version.bind`/`id.server` CHAOS + AXFR test-harness against declared secondaries at a polite cadence (default 5 min). Output as Prometheus metrics so third-party secondaries become observable.
- `nebula-dnstap-exporter`: consumes the server's DNSTAP stream and emits query-shape metrics (qname top-K via CMS, not a label) without hot-path impact.

#### Operator escape hatches

- Per-metric disable via config: `metrics.disabled = ["nebula_store_read_duration_seconds"]`. Rarely used; exists because sometimes a specific series is too expensive for a specific backend.
- Scrape endpoint ACL: `metrics.allow_cidrs = [...]`.
- `/metrics/lite` endpoint: trimmed set for extreme-low-overhead scraping (used only in pathological cases).
- OTLP push as an alternative to scrape (OpenTelemetry metrics over OTLP to a collector), with identical metric names.

### Logging

- `tracing` crate, JSON output by default, `jsonl` on stdout.
- Every request carries a trace/span id. Every zone change carries a deploy id. Every transfer carries a peer id + remote addr + TSIG key name (no secret).
- Log retention guidance in the Helm chart (30d hot, 180d cold) — addresses the "log retention increase" remediation item.

### Tracing

- OpenTelemetry-native (OTLP). Ship with a default collector config that targets Tempo/Jaeger.

---

## 7. Propagation verification (the big one)

This is the single feature that would have prevented both incidents. It MUST exist in v1.0.

### Mechanism

1. On every zone publish, the server records the new SOA serial and the declared list of downstream secondaries (from config or service discovery).
2. The **Propagation Verifier** polls each secondary's SOA over TCP/53 (or DoT if configured) at a configurable cadence (default 5s).
3. A deploy is considered **verified** only when every required secondary reports the new serial within the propagation SLA (default 60s, configurable).
4. If any secondary fails to converge, the deploy is marked **failed** — the CI/CD pipeline receives a non-zero exit and a structured failure reason.
5. Failure reasons surface the **actual wire error** (FORMERR, TSIG mismatch, timeout, refused) — not a generic "deploy failed".

### Requirements

- Operator can declare secondaries statically in config or dynamically via a discovery provider (DNS SRV, Consul, Kubernetes endpoints, AWS Route 53 delegation, Akamai API).
- Per-secondary policy: `required` (must converge) vs `best-effort` (report but don't block).
- Per-secondary transfer-method: `axfr`, `ixfr`, `none` (resolver-only check via public DNS).
- **Interop probe**: on registration, NebulaDNS performs a test AXFR of a synthetic zone against each secondary and records the peer's server software, version (via `version.bind` CHAOS query), and supported features. **This is the sentinel that would have detected Akamai's BIND 9.18 upgrade on the day it happened.**

---

## 8. React dashboard (`/ui`)

Stack: Vite + React 18 + TypeScript + Tanstack Query + Tanstack Router + Tailwind + shadcn/ui + Recharts.

OpenAPI-derived client (`openapi-typescript`) — **no hand-written types** for API models.

### Pages

1. **Overview** — global health at a glance. Red/amber/green tiles per zone. Total QPS, NXDOMAIN rate, FORMERR rate, propagation lag p95.
2. **Zones** — list, filter, search. Per-zone: current serial, record count, DNSSEC status, last deploy, next scheduled key rollover.
3. **Zone detail** — record editor (with diff preview), version history (git-like), rollback button, manual NOTIFY button, propagation panel (per secondary, last successful transfer, last attempt, last error).
4. **Secondaries** — health matrix. For each peer: last successful AXFR, software/version, FORMERR count, TSIG key in use, transfer duration p95. **The view that would have shown "we went from 6 working agents to 1" over 13 months.**
5. **Deploys** — timeline. Every deploy, who, what changed, propagation result, rollback link.
6. **Metrics** — embedded Grafana panels (iframe with auth), plus native drill-downs.
7. **DNSSEC** — key inventory, rollover schedule, next expiry, manual rollover trigger.
8. **Audit log** — filterable, exportable, hash-chain-verifiable.
9. **Settings** — RBAC, TSIG keys (no secrets displayed), discovery providers, alerting webhooks, feature flags.
10. **Swagger / ReDoc** — embedded API explorer, pre-authenticated with the UI session.

Accessibility: WCAG 2.1 AA. Dark mode default. Keyboard-first navigation.

---

## 9. Configuration

- **Schema-validated TOML** (primary), YAML (secondary) — both via the same serde schema.
- **Hot reload**: SIGHUP or `nebulactl reload`. Validation happens before swap; failed configs do not replace the running config.
- **Policy-as-code**: a config can declare policies (e.g., "every zone must have DNSSEC", "no wildcard A records outside approved zones") that are enforced on deploy.
- **Secrets**: never in the config file. Reference via `env:`, `file:`, `vault:`, `aws-sm:`, `gcp-sm:` URIs.

Example:

```toml
[server]
bind = ["0.0.0.0:53", "[::]:53"]
workers = "auto"  # = num_cpus

[transport.doh]
enabled = true
bind = "0.0.0.0:443"
tls = { cert = "file:/etc/nebula/tls.crt", key = "file:/etc/nebula/tls.key" }

[api]
bind = "127.0.0.1:8443"
auth = { oidc = { issuer = "https://sso.example.com" }, mtls = { ca = "file:/etc/nebula/ca.crt" } }

[metrics]
bind = "127.0.0.1:9090"

[[zone]]
name = "ticketmaster.com"
file = "zones/ticketmaster.com.toml"
dnssec = { algorithm = "ed25519", nsec3 = true }
secondaries = [
  { name = "akamai-a", addr = "23.14.128.185",  tsig = "akamai-xfer", required = true },
  { name = "akamai-b", addr = "104.122.95.88",  tsig = "akamai-xfer", required = true },
]
propagation = { sla_seconds = 60, policy = "all_required" }

[[secondary]]
name = "akamai-xfer"
# TSIG key loaded from Vault; never inline
key = "vault:kv/nebula/tsig/akamai-xfer"
```

---

## 10. Security

- `#![forbid(unsafe_code)]` in every crate that doesn't genuinely need it.
- `cargo-audit`, `cargo-deny`, `cargo-geiger` in CI.
- SBOM (CycloneDX) produced on every release.
- Reproducible builds (documented in `docs/reproducible-builds.md`).
- Supply chain: all dependencies pinned with `Cargo.lock`; dependency review gate on PRs.
- DDoS mitigation: EDNS cookies, per-client rate limits, RRL (Response Rate Limiting), TCP-only fallback under duress.
- DNSSEC private keys encrypted at rest; optional HSM/KMS integration.
- Audit log is append-only and hash-chained.

---

## 11. Performance targets (v1.0) — reliability and latency are the product

The numbers below are hard SLOs, not aspirations. Benchmarks gate every PR. Performance is framed reliability-first: we will never trade correctness for speed, but where safe Rust allows us to close the gap with C-class throughput, we take it.

### Latency (in-process, UDP simple A/AAAA, cache-hot)

| Percentile | Target | BIND 9.18 (ref) | Knot 3.3 (ref) | CoreDNS 1.11 (ref) |
|---|---|---|---|---|
| p50 | ≤ 40 µs | ~120 µs | ~55 µs | ~180 µs |
| p95 | ≤ 100 µs | ~300 µs | ~130 µs | ~450 µs |
| p99 | ≤ 150 µs | ~600 µs | ~250 µs | ~900 µs |
| p99.9 | ≤ 400 µs | ~2 ms | ~700 µs | ~6 ms (GC-induced) |

### Throughput (per node)

- **UDP simple answer**: ≥ 2,000,000 qps on 16-core x86_64 at ≤ 60% CPU.
- **UDP with DNSSEC online signing (Ed25519)**: ≥ 500,000 qps.
- **DoT**: ≥ 200,000 qps per core (rustls + session resumption).
- **DoH**: ≥ 150,000 req/s per core.
- **DoQ**: ≥ 250,000 req/s per core.
- **AXFR**: ≥ 1 GB zone / 10 seconds streamed.

### End-to-end SLOs

- p99 LAN end-to-end ≤ 1 ms.
- p99 WAN end-to-end dominated by network; we add ≤ 250 µs of server overhead.

### Footprint

- Cold start: ≤ 200 ms to serving traffic on a 10k-zone config.
- Idle RSS: ≤ 80 MB.
- RSS with 10M RRs loaded: ≤ 4 GB (benchmark against Knot).

### How we get there (engineering detail)

- **io_uring** (Linux ≥ 5.19) with `epoll` fallback; UDP `sendmmsg`/`recvmmsg` batching.
- **SO_REUSEPORT** with per-CPU-pinned worker threads; no global lock in the hot path.
- **Lock-free zone store**: immutable snapshot served to readers via `arc-swap`; writers swap atomically. Readers never block, never allocate.
- **Zone lookup**: compressed qp-trie / radix for O(label) descent; name-compression pointer table precomputed for wildcard matches.
- **Zero-copy encode**: response buffer built directly into the kernel-bound IO buffer.
- **No allocations on the hot path** in steady state — verified by `dhat` + a `jemalloc` hot-path allocation counter in the benchmark harness.
- **Huge pages** for the zone store (`madvise(MADV_HUGEPAGE)`); transparent when available.
- **CPU affinity + NUMA-aware** worker pinning.
- **DNSSEC online signing** via batched ECDSA/Ed25519 using `ring`/`aws-lc-rs`; signatures cached with a bounded LRU.
- **Response-rate-limiting (RRL)** uses an approximate sketch (CMS), not a per-source hashmap.
- **Deterministic GC absence**: Rust. Period.

### Benchmark harness

- `xtask bench` runs `criterion` micro-benchmarks, `dnsperf`/`flamethrower`/`resperf` macro-benchmarks, and a dedicated latency-tail harness using HDR histograms against a reference deployment.
- CI gates: any regression > 5% on p99 latency or throughput blocks the PR.
- Nightly: flamegraphs produced and published as artifacts.
- Quarterly: published comparative benchmark vs. BIND, Knot, PowerDNS, NSD, CoreDNS on identical hardware. Reproducibility kit in `bench/`.

### Reliability SLOs (the other half of performance)

- **Data-plane availability**: ≥ 99.999% per node (≤ 5 min/year unplanned).
- **Correctness**: zero malformed responses under fuzz — enforced by continuous fuzzing.
- **Zone integrity**: zero zone-version loss across crash + restart, verified by power-cut chaos tests (kill -9, disk full, OOM).
- **Mean time to detect (MTTD)** a failed propagation: ≤ 30 seconds.
- **Mean time to rollback (MTTR)** via UI/API: ≤ 2 minutes.

---

## 12. Testing strategy

1. **Unit tests** — every crate, >85% line coverage gate.
2. **Property tests** — `proptest` for parsers, serializers, zone diffing.
3. **Fuzz tests** — `cargo fuzz` targets for wire decoder, zone file parser, DNSSEC signer. Continuous via ClusterFuzzLite.
4. **Integration tests** — start a real NebulaDNS, run real `dig`/`drill`/`delv` against it, validate responses.
5. **Interop tests** — the test suite that would have caught incident 1326:
   - `nebula → bind918` AXFR must succeed.
   - `nebula → bind920` AXFR must succeed.
   - `nebula → knot3` AXFR must succeed.
   - `nebula → nsd4` AXFR must succeed.
   - `nebula → powerdns4` AXFR must succeed.
   - `bind918 → nebula` AXFR must succeed (we are also a good secondary).
   - These run in CI on every PR against a matrix of peer versions.
6. **Chaos tests** — `toxiproxy` between primary and secondaries: drop 10% packets, reorder, add latency, truncate mid-AXFR. Verify correct retry and alerting.
7. **End-to-end tests** — spin up a 3-node NebulaDNS + 2 BIND secondaries + 1 Knot secondary via `docker-compose`, run a full deploy, verify propagation, rollback, DNSSEC rollover.
8. **Load tests** — `dnsperf` + `flamegraph` in CI nightly.
9. **Conformance tests** — RFC test vectors from IETF drafts where available.

---

## 13. Deliverables

Every deliverable below is shipped in-tree, built in CI, published on every release, and signed (sigstore/cosign). All artifacts carry an SBOM (CycloneDX) and SLSA provenance attestations.

### 13.1. Binaries and packages

| Artifact | Form | Location in repo |
|---|---|---|
| Server binary | Static musl build (Linux x86_64, ARM64), glibc builds, macOS universal, FreeBSD amd64 | `target/release/nebuladns` |
| CLI binary | `nebulactl` — same platform matrix | `target/release/nebulactl` |
| `.deb` package | Ubuntu 22.04+, Debian 12+; includes systemd unit, apparmor profile, default config | `deploy/packages/deb/` |
| `.rpm` package | RHEL 9+, Rocky 9+, Amazon Linux 2023, openSUSE; includes systemd unit, SELinux policy | `deploy/packages/rpm/` |
| Homebrew formula | macOS | `deploy/packages/brew/nebuladns.rb` |
| FreeBSD port | `deploy/packages/freebsd/` |
| Scoop manifest (CLI only) | Windows — CLI only, server not supported on Windows | `deploy/packages/scoop/` |
| Install script | `curl -sSfL https://get.nebuladns.io \| sh` — detects OS/arch, verifies signature, installs binary + systemd unit | `deploy/install.sh` |
| Release tarball | `nebuladns-<version>-<target>.tar.gz` with binary + systemd unit + example config | GitHub Releases |

### 13.2. Systemd unit (required, ships in `.deb` and `.rpm`)

Location: `deploy/systemd/nebuladns.service`

```ini
[Unit]
Description=NebulaDNS authoritative DNS server
Documentation=https://docs.nebuladns.io
After=network-online.target
Wants=network-online.target
AssertPathExists=/etc/nebuladns/nebuladns.toml

[Service]
Type=notify
NotifyAccess=main
User=nebuladns
Group=nebuladns
ExecStart=/usr/bin/nebuladns --config /etc/nebuladns/nebuladns.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=2s
LimitNOFILE=1048576
LimitNPROC=65536

# Binding to :53 requires privileged ports
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

# Hardening
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=true
RestrictRealtime=true
LockPersonality=true
MemoryDenyWriteExecute=true
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallFilter=~@privileged @resources
ReadWritePaths=/var/lib/nebuladns /var/log/nebuladns

# Watchdog — server sends sd_notify() heartbeats
WatchdogSec=30s

[Install]
WantedBy=multi-user.target
```

Also ships: `nebuladns.socket` (optional socket activation for zero-downtime restart), `nebuladns-reload.timer` for scheduled zone-sync tasks, and a `sysusers.d` / `tmpfiles.d` drop-in.

### 13.3. Docker image (required, primary testing vehicle)

- **Base**: `gcr.io/distroless/static:nonroot` (or `scratch` for the static musl variant).
- **Size**: ≤ 25 MB compressed, ≤ 60 MB uncompressed.
- **Multi-arch**: `linux/amd64`, `linux/arm64`, `linux/arm/v7`.
- **Signing**: cosign-signed; SLSA level 3 provenance.
- **SBOM**: CycloneDX and SPDX attached as OCI referrers.
- **Tags**: `:latest`, `:v1.2.3`, `:v1.2`, `:v1`, `:sha-<gitsha>`, `:edge` (main branch).
- **Registries**: `ghcr.io/nebuladns/nebuladns`, `docker.io/nebuladns/nebuladns`, `quay.io/nebuladns/nebuladns`.
- **Healthcheck**: `HEALTHCHECK CMD ["/nebuladns", "healthcheck"]` — queries `/livez`.
- **Non-root**: runs as UID 65532, with `CAP_NET_BIND_SERVICE` granted at runtime (or bind to high ports + use host networking / a sidecar).
- **Dev/test variant**: `ghcr.io/nebuladns/nebuladns:dev` — includes `dig`, `drill`, `nebulactl`, and a shell for ease of testing.

Location: `deploy/docker/`. A `docker-compose.yml` in `deploy/docker/compose/` spins up a complete test rig: one primary NebulaDNS, two secondaries (BIND 9.18 + Knot 3.3), a Prometheus, a Grafana, and a client container with `dig` for interactive testing.

**Quickstart (the demo we will paste everywhere):**

```bash
docker run --rm -p 5353:53/udp -p 5353:53/tcp -p 8443:8443 \
  ghcr.io/nebuladns/nebuladns:latest --demo
# API:   https://localhost:8443/api/docs
# UI:    https://localhost:8443/
# Query: dig @localhost -p 5353 example.com
```

### 13.4. Helm chart (required, Kubernetes-native deployment)

Location: `deploy/helm/nebuladns/`. Published to an OCI Helm registry at `oci://ghcr.io/nebuladns/charts/nebuladns` and to a traditional chart repo at `https://charts.nebuladns.io`.

Chart features:

- **Three install modes** via a single `values.yaml` flag:
  1. `mode: standalone` — single `StatefulSet`, one replica, embedded store. Good for small/internal use.
  2. `mode: ha` — multi-replica `StatefulSet` with leader election (Kubernetes lease) for the primary role; readers scale horizontally.
  3. `mode: cluster-dns` — ships as a drop-in replacement for CoreDNS at `kube-system`, takes over cluster DNS with a migration path documented in `deploy/helm/nebuladns/docs/coredns-migration.md`.
- **Workload kinds**: `StatefulSet` (primary, for persistence), `Deployment` (stateless read replicas), `DaemonSet` (edge/anycast mode).
- **Services**: `Service` (ClusterIP + headless), `LoadBalancer` (for external), optional `Gateway`/`Ingress` for the control plane.
- **Networking**: native IPv4/IPv6 dual-stack, `NetworkPolicy` baked in, optional Cilium/Calico BGP peering.
- **Security**: runs as non-root, read-only root filesystem, drops all caps except `NET_BIND_SERVICE`, `PodSecurityStandard: restricted` compatible, `seccompProfile: RuntimeDefault`.
- **Storage**: optional `PersistentVolumeClaim` for journal/audit log (ReadWriteOnce, configurable storage class).
- **Secrets**: integrates with external-secrets-operator, Vault Agent Injector, AWS Secrets CSI, Azure Key Vault CSI.
- **Scheduling**: pod anti-affinity by default, configurable `topologySpreadConstraints`, `PodDisruptionBudget`.
- **Autoscaling**: `HorizontalPodAutoscaler` based on custom QPS metric (from `/metrics`), optional `VerticalPodAutoscaler` recommendations.
- **Observability**: `ServiceMonitor` (Prometheus Operator), `PodMonitor`, `PrometheusRule` with the alerts from §6, a `GrafanaDashboard` CR (if kube-prometheus-stack is installed), OTLP exporter configured by default.
- **Lifecycle**: `preStop` hook drains connections gracefully (stop accepting new, let in-flight complete, exit), `terminationGracePeriodSeconds: 60`.
- **Testing**: `helm test` hook runs a full smoke suite (`dig` the pod, hit `/livez`, verify `/metrics`, test a zone transfer).

Minimal install:

```bash
helm install nebuladns oci://ghcr.io/nebuladns/charts/nebuladns \
  --namespace nebuladns --create-namespace \
  --set mode=ha --set replicas=3
```

### 13.5. Kubernetes-native operator and CRDs (required)

Location: `deploy/k8s-operator/`. This is the single biggest differentiator vs. every other authoritative DNS server — **GitOps-native authoritative DNS**.

Shipped as:

- A separate `nebula-operator` binary and container image.
- A Helm chart at `deploy/helm/nebula-operator/`.
- An OLM bundle (`deploy/k8s-operator/bundle/`) for OperatorHub.
- Installable via `nebulactl k8s install-operator`.

CRDs (all `apiVersion: dns.nebuladns.io/v1`):

| Kind | Purpose |
|---|---|
| `Zone` | Declare an authoritative zone (name, DNSSEC policy, TTL defaults, nameservers, secondaries) |
| `Record` | Individual RRsets owned by a `Zone` (A, AAAA, CNAME, MX, TXT, SRV, CAA, etc.) |
| `Secondary` | A downstream server (BIND, Akamai, Route 53, another NebulaDNS) with TSIG key ref and health policy |
| `TsigKey` | TSIG key material (pulled from Secret/ExternalSecret, never inlined) |
| `DeployGate` | Declare required propagation criteria (which secondaries, SLA) before a deploy is considered successful |
| `Policy` | Organization-wide policies (e.g., "every zone must have DNSSEC", "no wildcards in production") |
| `ZoneExport` | Read-only mirror of zone state as a CRD (for GitOps diffing) |

Example:

```yaml
apiVersion: dns.nebuladns.io/v1
kind: Zone
metadata:
  name: ticketmaster-com
  namespace: dns-prod
spec:
  name: ticketmaster.com
  dnssec:
    algorithm: ed25519
    nsec3: true
    keyStorage:
      kmsRef: { name: kms-prod }
  defaultTtl: 300
  nameservers:
    - ns1.ticketmaster.com
    - ns2.ticketmaster.com
  secondaries:
    - name: akamai-a
    - name: akamai-b
  deployGate:
    allRequired: true
    slaSeconds: 60
---
apiVersion: dns.nebuladns.io/v1
kind: Record
metadata:
  name: www-ticketmaster-com
  namespace: dns-prod
spec:
  zoneRef: { name: ticketmaster-com }
  name: www
  type: A
  ttl: 60
  records:
    - 192.0.2.10
    - 192.0.2.11
```

Reconciliation guarantees:

- Every CRD change is translated into API calls to NebulaDNS.
- The operator respects `DeployGate` — the `Zone` does not transition to `Ready` until propagation is verified.
- Drift detection runs every 60s; drift between CRD and server state is surfaced as a `Condition` on the CRD and as a Prometheus metric.
- Deletion is two-phase: records go to a `PendingDelete` state, TTL must expire, then they are purged. Prevents dangling references.

### 13.6. ExternalDNS provider

We ship a first-class provider plugin for [kubernetes-sigs/external-dns](https://github.com/kubernetes-sigs/external-dns) at `integrations/external-dns/`. A Service or Ingress annotated with `external-dns.alpha.kubernetes.io/hostname: foo.example.com` results in a `Record` CRD, which the operator applies to NebulaDNS via the API, which then verifies propagation.

### 13.7. Cluster DNS mode (CoreDNS replacement)

`mode: cluster-dns` in the Helm chart turns NebulaDNS into a drop-in replacement for CoreDNS for Kubernetes internal DNS:

- Serves `cluster.local` and PTR zones from the Kubernetes API (via a watch-based backend: `nebula-kube-backend`).
- Supports the same `Corefile` plugin semantics (rewrite, cache, forward, hosts) via a translation layer; `nebulactl import --from corefile Corefile` produces a NebulaDNS config.
- Passes the upstream CoreDNS conformance test suite in CI.
- Tail-latency claim: under 100k qps pod-to-pod, p99.9 ≤ 400 µs vs. CoreDNS's ~6 ms (GC-induced). Benchmark reproducibility kit in `bench/coredns-comparison/`.

### 13.8. Other deliverables

| Artifact | Form | Location |
|---|---|---|
| Terraform module | Manages zones, secondaries, TSIG keys via API | `deploy/terraform/` |
| Ansible collection | `nebuladns.dns` namespace | `deploy/ansible/` |
| Pulumi package | `@nebuladns/pulumi` | `deploy/pulumi/` |
| OpenAPI spec | `docs/openapi.yaml`, regenerated on build | `docs/openapi.yaml` |
| gRPC .proto files | `docs/proto/` | `docs/proto/` |
| Grafana dashboards | JSON, imported by ConfigMap | `deploy/grafana/*.json` |
| Prometheus alerts | `deploy/prometheus/alerts.yaml` | `deploy/prometheus/` |
| Runbooks | One per alert | `docs/runbooks/` |
| Migration guide from TinyDNS | `docs/migration-from-tinydns.md` | `docs/` |
| Migration guide from BIND | `docs/migration-from-bind.md` | `docs/` |
| Migration guide from CoreDNS | `docs/migration-from-coredns.md` | `docs/` |
| Backstage plugin | Service catalog integration | `integrations/backstage/` |

---

## 13.X. Kubernetes compatibility & readiness matrix

A dedicated, hard-enforced set of conformance checks. Each line must pass in CI against a `kind`/`k3d`/`minikube` cluster before release.

| Check | Requirement |
|---|---|
| Runs on `kind`, `k3d`, `minikube`, `EKS`, `GKE`, `AKS`, `OpenShift`, `Rancher/RKE2`, `Talos` | All green in CI matrix |
| Pod Security Admission `restricted` profile | Pass without waivers |
| Runs with read-only root filesystem | Yes |
| Runs as non-root | Yes (UID 65532) |
| IPv6-only clusters | Supported |
| Dual-stack clusters | Supported |
| `NetworkPolicy` default-deny compatible | Yes, sample policy shipped |
| Works behind Gateway API | Yes, `HTTPRoute` example shipped |
| Works with cert-manager for control plane TLS | Yes |
| Works with external-secrets-operator | Yes |
| Passes `kubeconform` on all manifests | Yes |
| Chart passes `helm lint --strict` and `ct lint` | Yes |
| Operator passes `operator-sdk scorecard` | Yes |
| Passes CNCF conformance for cluster-DNS mode | Yes |
| Graceful shutdown within `terminationGracePeriodSeconds` | Yes, verified by chaos test |
| Rolling updates cause zero dropped queries | Verified by `dnsperf` during rollout |
| Scales horizontally without coordination on read path | Yes (snapshot replication) |
| Works with Karmada / Cluster API for multi-cluster | Yes, documented |

---

## 14. Migration from TinyDNS (our path off djbdns)

A dedicated subcommand:

```
nebulactl import --from tinydns --data ./data --out ./zones/
```

- Parses djbdns `data` file directly.
- Generates equivalent NebulaDNS zone TOML.
- Preserves record order and TTLs.
- Produces a diff report vs. current `data.cdb`.
- Includes a live-traffic shadow mode: NebulaDNS answers in parallel and compares to TinyDNS for N hours; divergences are reported but not served.

---

## 14.5. High availability, multi-region failover, and cross-region sync

HA is not an optional feature — it is a core correctness property. A single-node authoritative DNS server is never the design target. Every topology below is tested, documented, and shipped.

### 14.5.1. HA modes

| Mode | Topology | Target use case | Failure domain |
|---|---|---|---|
| **Single node** | 1 primary, local embedded store | Dev, lab | Node |
| **HA-Local** | 3-node cluster in one region, Raft-based leader election, shared replicated state | Regional deployments, small production | Node or AZ |
| **HA-Multi-AZ** | 3+ nodes spread across ≥ 3 availability zones, 1 leader, N-1 hot standbys serving reads | Production in one region | AZ |
| **Multi-Region Active-Passive** | 1 primary region, 1+ DR regions, async replicated, promotable in < 60s | DR, compliance, geo-proximity | Region |
| **Multi-Region Active-Active** | Multiple regions, all writable, CRDT/last-writer-wins per RRset with vector clocks + monotonic serial authority | Global services, lowest-latency writes | Region, partition |
| **Global anycast edge** | DaemonSet / edge binaries at PoPs, read-only replicas fed from a control region | Public-facing authoritative service | PoP, network |

### 14.5.2. State replication architecture

- **Control plane** uses **Raft** (via `openraft` or `raft-rs`) for strongly consistent writes of zone data, deploy metadata, and audit log within a region. Leader election via Raft, hot standbys serve reads from a consistent snapshot.
- **Data plane** reads from an immutable, per-node snapshot of the zone store; replication to followers is a WAL stream (Raft log → per-follower materialization).
- **Cross-region replication** uses an **async log-ship** protocol over mTLS:
  - Every committed zone version produces a signed changeset (content-addressed, Merkle-linked).
  - DR regions subscribe to the changeset stream; lag is exposed as `nebula_replication_lag_seconds{source_region,dest_region}`.
  - Active-active mode: each region has authority over the serial range it allocates (partitioned serial space); conflicts on overlapping RRsets are resolved by `(logical_clock, region_id)` with explicit conflict surfacing in the UI.
- **Write quorum**: configurable per zone. Defaults: Raft majority in-region for HA-Local; in Active-Active, "write accepted locally + async-shipped to N peers" with a `sync: quorum | local | all` policy per zone.
- **Split-brain prevention**: Raft in-region prevents it. Cross-region in Active-Active uses leases + fencing tokens; a region that loses quorum transitions to read-only with a visible banner in the UI.

### 14.5.3. Failover

- **Automatic failover**:
  - In-region leader failure: new leader elected within ≤ 5 seconds (Raft default timings).
  - Region failure: DR promotion is **automated with an operator-approved gate by default** (prevents flapping promotions); a configurable policy allows fully automatic if the operator opts in.
  - Data plane nodes continue serving from their last-known-good snapshot for the duration of `stale_ok_seconds` (default 86400); queries are answered with correct data but the `Propagation` panel shows "degraded: control plane unreachable."
- **Manual failover**: single API call: `POST /api/v1/regions/{region}/promote`. Requires two-operator approval by default (configurable).
- **Failback**: explicit, operator-driven. Never automatic. Re-syncs via replay of the changeset log from the new primary.
- **Pre-flight checks before promotion**: DR region must have replication lag ≤ `max_promotion_lag_seconds` (default 30s) and must have passed a synthetic query suite within the last 5 minutes. Otherwise promotion requires `--force` with an audit-logged justification.

### 14.5.4. RTO and RPO targets

| Topology | RTO (time to serve) | RPO (data loss) |
|---|---|---|
| HA-Local (AZ loss) | ≤ 10 s | 0 (Raft committed) |
| Multi-AZ (2 AZ loss) | ≤ 30 s (read-only if quorum lost) | 0 |
| Multi-Region A/P (region loss) | ≤ 60 s with auto-promote, ≤ 5 min with manual | ≤ 5 s of changes (async ship) |
| Multi-Region A/A (region loss) | ≤ 10 s (other regions still writable) | 0 for writes that reached local quorum |

### 14.5.5. Global anycast & edge

- **Anycast** at the BGP layer (in-process `holo`/`rustybgp` or sidecar `bird2`/`gobgp`); health-driven withdrawal.
- **Geo-DNS** at the application layer: `Views` allow returning different answers per client network.
- **Edge mode**: `DaemonSet` at PoPs or edge nodes with a read-only store kept in sync via the changeset stream. Edge nodes serve traffic even when the control region is unreachable (stale-ok).
- **Latency-based routing**: optional plugin that uses RIPE/MaxMind data to return topologically-nearest answers.

### 14.5.6. Multi-region testing

- CI spins up a 3-region simulated cluster (`kind` clusters + `netem` between them) and runs a chaos suite: partition a region, kill the leader, corrupt the replication log, inject replication lag, clock skew, etc. All must converge to correct state.
- A quarterly **game day** exercise is codified in `docs/runbooks/game-day.md`. Running it is a release blocker.

### 14.5.7. Backup and restore

- **Continuous backup**: every committed zone version is shipped to S3-compatible object storage (versioned bucket, optional object-lock) by default.
- **Point-in-time restore**: any zone to any prior version via `nebulactl zone restore <zone> --at <timestamp>` or via the UI.
- **Disaster recovery kit**: `nebulactl dr export-bundle` produces a portable, encrypted bundle (zones + audit log + TSIG key metadata, **not** private key material unless explicitly opted in). Tested quarterly via restore-into-fresh-cluster.

---

## 14.6. CI/CD, release engineering, and automation

A first-class delivery pipeline is required from day one. The pipeline itself is a product deliverable — it is what makes the reliability guarantees credible.

### 14.6.1. Source control and branching

- **Monorepo**: server + CLI + UI + operator + Helm + Terraform + Ansible + Backstage in one Cargo/Turborepo workspace.
- **Trunk-based development** with short-lived feature branches.
- **Protected `main`**: signed commits required, linear history, mandatory CI green, mandatory review from `CODEOWNERS`.
- **Release branches**: `release/v1.x` cut at GA; backports via cherry-pick only.
- **Conventional Commits** enforced (`commitlint`) — drives automated changelog + semver.
- **Automated dependency updates**: Renovate (with grouped PRs), `cargo update --dry-run` gate, weekly `cargo audit`.

### 14.6.2. CI pipeline (runs on every PR)

Platform: GitHub Actions (primary), with portability to GitLab CI and Buildkite documented.

Stages (fail fast, run in parallel where possible):

1. **Lint & format**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo deny check`, `eslint`, `prettier`, `helm lint`, `kubeconform`, `shellcheck`, `hadolint` (Dockerfiles).
2. **Build**: matrix of `linux-amd64-musl`, `linux-arm64-musl`, `linux-amd64-gnu`, `macos-universal`, `freebsd-amd64`. Incremental caching via `sccache` + `actions/cache`.
3. **Unit tests**: `cargo nextest run --all-features` with coverage via `cargo-llvm-cov` (gate: ≥ 85% line coverage on changed crates).
4. **Property & fuzz-smoke**: every `cargo fuzz` target gets a 60-second smoke run; full runs happen continuously on OSS-Fuzz / ClusterFuzzLite.
5. **Integration tests**: spin up NebulaDNS in a container, query with `dig`, exercise the API via OpenAPI-derived client.
6. **Interop matrix**: AXFR/IXFR/NOTIFY against BIND 9.18 / 9.20 / Knot 3.3 / NSD 4.x / PowerDNS 4.x / djbdns 1.05 / CoreDNS 1.11. (This is the test that would have caught incident 1326.)
7. **Chaos smoke**: toxiproxy-driven packet loss, reorder, latency, truncated AXFR. Asserts no data corruption and correct alerting.
8. **Performance gate**: `criterion` + `dnsperf` runs with HDR histogram; regressions > 5% on p99 latency or throughput block the PR.
9. **Security scans**: `cargo audit`, `cargo deny`, `trivy` on container images, `grype` for SBOM vulns, `semgrep` for Rust custom rules, `gitleaks` for secrets, CodeQL.
10. **Kubernetes checks**: `helm template | kubeconform`, operator `scorecard`, `kind` + `k3d` smoke install, chart upgrade test (`v{n-1}` → `v{n}`).
11. **Docs build**: `mkdocs build --strict`, OpenAPI spec validation, broken-link check.
12. **Frontend tests**: `vitest`, `playwright` e2e against a live backend.
13. **Migration tests**: `nebulactl import` against a corpus of real-world BIND, TinyDNS, CoreDNS configs.

All runs produce artifacts: flamegraphs, coverage reports, SBOMs, benchmark deltas, test videos (Playwright). Artifacts retained 90 days.

### 14.6.3. Release pipeline

Triggered by pushing a tag `v*.*.*` on `main` or a release branch.

1. Re-run full CI.
2. Build all binaries (cross-compiled, reproducibly).
3. Build container images, multi-arch, signed (cosign keyless via GitHub OIDC), SBOM attached, SLSA-3 provenance attested.
4. Build packages (`.deb`, `.rpm`, Homebrew, FreeBSD port).
5. Publish to: GitHub Releases, OCI registries (ghcr/docker/quay), apt/yum repos (`apt.nebuladns.io`, `yum.nebuladns.io`), OCI Helm registry + `charts.nebuladns.io`, crates.io (library crates), npm (TypeScript API client).
6. Publish docs to `docs.nebuladns.io` (versioned).
7. Auto-generate the changelog from Conventional Commits; populate the release body.
8. Auto-file a PR to bump the Homebrew tap and the Helm chart index.
9. Announce: release notes posted to a dedicated changelog feed, RSS, and an announcements-only Slack/Discord webhook.

**All release artifacts are signed and verifiable.** The public signing root is documented in `SECURITY.md` with rotation policy.

### 14.6.4. Deployment automation for consumers

- **GitOps examples** shipped in `examples/gitops/` — Flux and ArgoCD configs that deploy NebulaDNS via the Helm chart plus `Zone`/`Record` CRs pulled from Git.
- **Terraform module**: idempotent, with `terraform plan` showing zone diffs.
- **Pulumi package**.
- **Ansible collection** with canary-aware playbooks.
- **Canary promotion controller**: a small sidecar that watches `/metrics` and rolls a Kubernetes `Deployment` forward automatically based on SLO budget burn. Integrates with Argo Rollouts and Flagger.
- **Drift detection CLI**: `nebulactl drift check` — compares in-cluster CRD state to live server state, reports deltas, optionally auto-reconciles.

### 14.6.5. Environments and promotion

Reference layout shipped in `docs/environments.md`:

- `dev` → `staging` → `canary` → `prod`.
- Each environment has its own NebulaDNS cluster; Git branch strategy drives promotion via ArgoCD.
- **Canary**: 1% of traffic for 30 minutes, gated by SLO burn rate (error budget policy: halt if 5m burn > 2x budget).
- **Zone-level canaries**: a single zone can be canaried to a new server version via the secondary mechanism — the new version subscribes as a secondary, is validated, then promoted.

---

## 14.7. Best practices for maintaining DNS software (organizational commitments)

A DNS server is a single point of failure for an entire business. These practices are not optional; they are shipped in-tree as enforceable policy.

### 14.7.1. Protocol hygiene

- **Continuous interop monitoring**: a scheduled CI job runs the interop matrix **daily** against the latest releases of BIND, Knot, PowerDNS, NSD, CoreDNS, Unbound (as client). Breakage opens a ticket automatically. This is the direct policy response to incident 1326.
- **Peer software fingerprinting in production**: the propagation verifier records every peer's `version.bind` CHAOS response; sudden version changes in any declared secondary fire an alert. **This is the sentinel that would have caught Akamai's silent BIND 9.18 rollout 13 months earlier.**
- **RFC-conformance test suite** re-run on every dependency/toolchain bump, not just on PRs.
- **Wire-format change budget**: any PR touching `nebula-wire` requires two `CODEOWNERS` approvals, a fuzz-corpus update, and interop-matrix green.

### 14.7.2. Observability discipline

- **Golden signals SLO**: every release ships with SLO definitions in `docs/slos.md`; the release is blocked if recent production data shows SLO burn.
- **Runbooks are code**: every `PrometheusRule` alert has a matching `docs/runbooks/<alert>.md`. CI enforces the link exists and is non-empty.
- **Incident retro template**: `docs/incidents/TEMPLATE.md`; every P0/P1 results in a published retro within 14 days.
- **Post-deploy verification is mandatory**: any deploy that skips the propagation gate requires a named-operator override that is audit-logged.

### 14.7.3. Security discipline

- **Responsible disclosure**: `SECURITY.md` with a GPG key and a 90-day coordinated-disclosure policy.
- **CVE process**: a documented triage + fix SLO (critical ≤ 48h, high ≤ 7d, medium ≤ 30d).
- **Bug bounty**: via HackerOne or equivalent, scoped to the server, API, and operator.
- **Third-party audit** before v1.0 GA and annually thereafter. Audit reports published.
- **Dependency review**: new dependencies require a written rationale (where, why, license, maintenance signal, alternatives considered).
- **Secret rotation**: TSIG keys, DNSSEC keys, API tokens rotate on a schedule; the schedule is expressed as Prometheus alerts that fire before expiry.
- **Compromise playbook**: `docs/runbooks/compromise-response.md` with step-by-step key rotation, zone rollback, and customer communication procedures.

### 14.7.4. Change-management discipline

- **No un-dry-runnable operations**: every API write supports `?dry_run=true` and returns the diff.
- **Two-person approval** for: DNSSEC algorithm changes, zone deletion, region promotion, policy deviations.
- **Blast radius limits**: policy engine can cap the fraction of records changed in one deploy (e.g., "no single deploy may modify > 10% of RRsets in a production zone"). Overrides require audit-logged justification.
- **Scheduled maintenance windows** and named freeze periods (e.g., "Black Friday freeze") are declarable in config and enforced by the API.

### 14.7.5. Long-term maintenance

- **Supported versions**: three concurrent release lines, each supported for 18 months with security patches.
- **LTS program**: every other `v1.x` is an LTS with 3 years of security patches.
- **Deprecation policy**: minimum two minor releases' notice; `Deprecation` HTTP header and CLI warnings.
- **Public roadmap**: `docs/roadmap.md`, updated quarterly, driven by GitHub issues tagged `kind/roadmap`.
- **Contribution guide**: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `GOVERNANCE.md`, PR template, issue templates, `good-first-issue` curation.
- **Telemetry** (opt-in only, anonymous): helps prioritize maintenance.

---

## 15. Milestones (proposed)

| Phase | Scope | Duration |
|---|---|---|
| **M0: Skeleton** | Workspace, CI, observability spine, health endpoints, systemd unit stub, Dockerfile, Helm chart skeleton | 2 weeks |
| **M1: Wire + Zone** | Wire codec, zone parser, fuzz harness, minimal UDP/TCP listener | 3 weeks |
| **M2: Auth zone** | In-memory authoritative answering, EDNS0, basic RBAC on API | 3 weeks |
| **M3: Transfers** | AXFR/IXFR/NOTIFY/TSIG + interop test matrix | 4 weeks |
| **M4: DNSSEC** | Online signing, key rollover, HSM/KMS | 3 weeks |
| **M5: Control plane** | Full REST API, gRPC, Swagger, CLI, audit log | 4 weeks |
| **M6: Verifier** | Propagation verifier, deploy gate, alerting | 2 weeks |
| **M7: HA + multi-region** | Raft/leader election, cross-region replication, failover, sync | 4 weeks |
| **M8: Kubernetes operator** | CRDs, operator, ExternalDNS provider, cluster-DNS mode | 4 weeks |
| **M9: UI** | React dashboard, all ten pages | 5 weeks |
| **M10: Packaging** | `.deb`, `.rpm`, Homebrew, FreeBSD, install script, signed artifacts | 2 weeks |
| **M11: CI/CD maturity** | Full release pipeline, SLSA-3 provenance, cosign, nightly chaos, canary harness | 2 weeks |
| **M12: Hardening** | Fuzzing at scale, load tests, chaos, third-party security review | 4 weeks |
| **M13: Migration** | TinyDNS/BIND/CoreDNS importers, shadow-mode, docs | 2 weeks |
| **M14: GA** | v1.0.0 release | 1 week |

Approximately 10–12 months for a small team (4–6 engineers) at a sustainable pace.

---

## 16. Open questions (answer before coding begins)

1. **Anycast topology** — do we BGP-speak from the node, or rely on an external load balancer / DSR? (Affects `nebula-server` shape.)
2. **HSM vs. software keys** — baseline for DNSSEC key storage at GA?
3. **Cluster mode for v1** — do we ship multi-primary raft consensus, or is "one primary, many secondaries" enough for v1?
4. **Managed-DNS story** — do we keep the SaaS control plane out of scope for v1, or is there a minimal multi-tenancy sketch we want from day one?
5. **Akamai integration** — do we want a native Akamai Fast DNS provider (API-level, bypassing AXFR) as a first-class secondary adapter?
6. **Licensing** — Apache-2.0 vs. AGPL-3.0 vs. dual. Affects contributor/vendor adoption.
7. **Telemetry-at-rest** — do we ship an opinionated Loki+Tempo+Prometheus stack in `deploy/`, or stop at dashboards and let operators bring their own?

---

## 17. What success looks like

We declare NebulaDNS v1.0 a success when, simulating the conditions of incident 1326 in staging:

1. A downstream secondary returns FORMERR on AXFR.
2. Within **30 seconds**, the dashboard surfaces the failure with the exact wire error and peer identity.
3. Within **60 seconds**, the deploy pipeline reports failure (not "complete").
4. An on-call engineer can, without SSH, roll back the zone via the UI in under **2 minutes**.
5. The interop test matrix would have failed in CI before this ever shipped.

**If all five are true, incidents 1273 and 1326 cannot recur in the same form.** That is the bar.

---

## Instructions to the AI implementer

When implementing this project, you MUST:

1. Treat each section of this document as binding design intent. Deviations require written justification in `docs/decisions/` (ADRs).
2. Implement the interop test matrix (§12.5) **before** writing AXFR code. The tests come first.
3. Implement `/metrics`, `/livez`, `/readyz`, and structured logging **in M0**, not as a retrofit. Metrics are always-on; the metrics-overhead benchmark (§6 Design contract) runs in CI from day one.
4. Generate the OpenAPI spec from code (via `utoipa`), never hand-maintain it. **API-first** is load-bearing: every operator action is an API call before any CLI or UI code exists.
5. Use `#![forbid(unsafe_code)]` unless a crate genuinely needs `unsafe` — and if it does, justify it in the crate's README.
6. Write ADRs for every non-obvious choice (storage backend, async runtime, TLS library, DNSSEC algorithm defaults, Raft vs. alternative consensus).
7. Pair each Prometheus alert with a runbook entry in `docs/runbooks/` in the same PR.
8. Do not ship a feature without a test, a metric, and a doc section.
9. Ship the systemd unit, Dockerfile, and Helm chart skeleton **in M0** alongside the first `/livez` endpoint — these are not "packaging work at the end"; they are the deployment contract.
10. HA, multi-region replication, and the Kubernetes operator are in-scope for v1.0 (M7/M8), not post-GA. A single-node-only release is not acceptable.

Start by producing: (a) a Cargo workspace skeleton matching §3, (b) CI config per §14.6.2, (c) the `nebula-wire` crate with fuzz harness, (d) M0 deployment artifacts (systemd unit, Dockerfile, Helm chart skeleton), and (e) a one-page architecture diagram in `docs/architecture.md`. Then stop and request review before proceeding to M1.
