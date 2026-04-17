<!-- NebulaDNS README -->

<h1 align="center">🛰️ NebulaDNS</h1>
<p align="center">
  <strong>A modern, observable, API-first authoritative DNS server written in safe Rust.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" /></a>
  <img alt="MSRV" src="https://img.shields.io/badge/rustc-1.80+-orange.svg" />
  <img alt="Status" src="https://img.shields.io/badge/status-M1%20(Wire%20%2B%20Zone)-amber.svg" />
  <img alt="Unsafe" src="https://img.shields.io/badge/unsafe-forbidden-brightgreen.svg" />
  <img alt="Platforms" src="https://img.shields.io/badge/linux%20%7C%20macOS%20%7C%20freebsd-supported-informational.svg" />
</p>

NebulaDNS replaces 2001-era daemons (TinyDNS / djbdns) and 40-year-old C codebases
(BIND) with a single small binary that ships with metrics, a control plane, and a
Kubernetes operator — so the next failure is detected in seconds, not by customers.

- **Why another DNS server?** See the [competitive analysis](#why-nebuladns).
- **Try it in 2 minutes?** Jump to [Quickstart](#quickstart).
- **Landing page preview?** Open [`index.html`](index.html) in a browser.

---

## Table of contents

- [Status](#status)
- [Features](#features)
- [Quickstart](#quickstart)
  - [Docker Compose](#docker-compose)
  - [Helm / Kubernetes](#helm--kubernetes)
  - [systemd / Debian / Ubuntu](#systemd--debian--ubuntu)
  - [Build from source](#build-from-source)
- [Why NebulaDNS](#why-nebuladns)
- [Architecture](#architecture)
- [Observability](#observability)
- [Operations](#operations)
- [Security](#security)
- [Repository layout](#repository-layout)
- [Contributing](#contributing)
- [License](#license)

---

## Status

**M1 (Wire + Zone)** — the server answers real DNS queries over UDP and TCP from a TOML
zone file and exposes labelled Prometheus metrics for every query.

| Milestone | Scope | State |
|-----------|-------|-------|
| **M0** | Workspace, CI, `/livez`, `/readyz`, `/metrics`, systemd unit, Docker image, Helm chart skeleton, `nebula-wire` fuzz harness | ✅ Done |
| **M1** | Full RFC 1035 codec, name compression, RR types, TOML zone loader, UDP + TCP listeners, data-plane metrics | ✅ Done |
| **M2** | Wildcards, CNAME chasing, glue, RFC 2308 negative responses, EDNS negotiation, RRL | Next |
| **M3** | AXFR / IXFR / NOTIFY / TSIG + daily interop matrix | Planned |
| **M4** | Online DNSSEC signing (Ed25519 / ECDSA / RSA), HSM + KMS | Planned |
| **M5** | Full REST API, gRPC, Swagger, CLI expansion, audit log | Planned |
| **M6** | Propagation verifier + deploy gate | Planned |
| **M7** | HA + multi-region replication (Raft + async log-ship) | Planned |
| **M8** | Kubernetes operator + CRDs, ExternalDNS, CoreDNS drop-in | Planned |
| **M9** | React dashboard | Planned |

Full milestone schedule in [`PROJECT_PROMPT.md §15`](PROJECT_PROMPT.md).

---

## Features

### Today (M0 + M1)

| Feature | Detail |
|---------|--------|
| RFC 1035 wire codec | Header, name (with compression + cycle detection), question, RR, message. Strict parsing — errors like `QdCountMismatch` are explicit variants, not silent corruption. |
| RR types | A, AAAA, NS, CNAME, SOA, MX, TXT, PTR, SRV, CAA; unknown types pass through verbatim (good-secondary behaviour). |
| EDNS(0) | OPT pseudo-RR decoded and re-emitted; DO bit parsed. |
| RFC 4035 flag bits | AD and CD bits recognised (caught the first time `dig` queried us). |
| Zone loader | Native TOML schema with `deny_unknown_fields`; hash-indexed RRsets with case-insensitive lookup. |
| Authoritative answer path | NOERROR with answers / NODATA / NXDOMAIN + SOA / REFUSED for out-of-zone. |
| UDP + TCP listeners | tokio-based; RFC 7766 length-prefix framing on TCP; UDP truncation with `TC=1` when responses exceed 512 bytes. |
| Zone registry | `arc-swap`-backed lock-free snapshot; readers never block; zones replaced atomically. |
| Control plane | `/livez`, `/readyz`, `/api/v1/version` served on a separate bind. |
| Metrics | Always-on Prometheus with compile-time-bounded labels (`proto`/`qtype`/`rcode`). Counter + histogram + drop reasons. Zero hot-path allocation. |
| Logging | Structured JSON via `tracing`; configurable filter. |
| systemd integration | `Type=notify` with `sd_notify` READY + watchdog; graceful SIGTERM shutdown. |
| Container | Distroless non-root image, multi-arch, ~25 MB compressed. Built-in `HEALTHCHECK`. |
| CLI | `nebulactl version` / `nebulactl health`. |
| Fuzz harness | Four `cargo-fuzz` targets (header / question / name / message). 60 s smoke in CI on every PR. |
| Property tests | `proptest` roundtrip for header, question, message. |
| Workspace hygiene | `#![forbid(unsafe_code)]` in every crate. Clippy `-D warnings` with `pedantic` + `nursery`. `cargo deny`, `cargo audit`, `rustfmt` in CI. |

### Deployment and release engineering

| Feature | Detail |
|---------|--------|
| Helm chart | StatefulSet, headless Service, LB Service (DNS), PodDisruptionBudget, NetworkPolicy, ServiceMonitor, PrometheusRule, `helm test` smoke pod. Zones mounted via ConfigMap from `values.yaml`. |
| Docker Compose | NebulaDNS + Prometheus + Grafana (pre-provisioned dashboard) + optional BIND 9.18 peer. One-shot `smoke` profile runs `dig` + curl assertions. |
| Ansible deploy | Idempotent role with staged binary install, pre-flip smoke test, atomic symlink swap, `CAP_NET_BIND_SERVICE` grant, config validation, health probe, old-binary pruning. |
| Rollback | `ansible-playbook rollback.yml` flips the symlink back to the previously staged binary — no downloads, seconds per host. |
| Release pipeline | `deploy.yml`: test gate → cross-compiled musl build (amd64 + arm64) → Ansible deploy with `--check --diff` preview in production. |
| Docker test workflow | `test-docker.yml`: builds the image, spins up compose, runs real `dig` queries from the runner, scrapes `/metrics`. |
| k3s test workflow | `test-k3s.yml`: spins up `k3d`, loads the image, `helm install`, `helm test`, port-forward + `dig`. |
| CI gates | Fmt, clippy, nextest, `cargo deny`, `cargo audit`, fuzz-smoke, helm lint + kubeconform, hadolint, shellcheck, ansible-lint + syntax-check. |

---

## Quickstart

### Docker Compose

The fastest way to see NebulaDNS serve real traffic. Includes Prometheus, Grafana with a
pre-provisioned dashboard, and a dig-based smoke container.

```bash
git clone https://github.com/nebuladns/nebuladns.git
cd nebuladns

docker compose -f deploy/docker/compose/docker-compose.yml up --build -d

# Real DNS over UDP and TCP
dig @127.0.0.1 -p 5353 www.example.com A +short
# 192.0.2.10
# 192.0.2.11

dig @127.0.0.1 -p 5353 +tcp example.com SOA +short
dig @127.0.0.1 -p 5353 nope.example.com A | grep status:     # NXDOMAIN
dig @127.0.0.1 -p 5353 foo.not-our-zone.test A | grep status: # REFUSED

# Metrics + dashboards
curl -s http://127.0.0.1:9090/metrics | grep nebula_dns_queries
open http://127.0.0.1:3000   # Grafana  (admin / admin)
open http://127.0.0.1:9091   # Prometheus

# Automated smoke suite (dig + /metrics assertions). Exits non-zero on failure.
docker compose -f deploy/docker/compose/docker-compose.yml --profile smoke run --rm smoke

# Teardown
docker compose -f deploy/docker/compose/docker-compose.yml down -v
```

### Helm / Kubernetes

```bash
# Basic install (from the repo — OCI chart publishing lands with v1.0).
helm install nebuladns deploy/helm/nebuladns \
  --namespace nebuladns --create-namespace

# Or with a real zone inlined in values:
cat <<'EOF' > /tmp/values.yaml
zones:
  - name: "example.com"
    content: |
      origin = "example.com."
      default_ttl = 300
      [soa]
      mname = "ns1.example.com."
      rname = "hostmaster.example.com."
      serial = 2026041701
      refresh = 10800
      retry = 3600
      expire = 604800
      minimum = 300
      [[records]]
      name = "www"
      type = "A"
      value = "192.0.2.10"
EOF
helm install nebuladns deploy/helm/nebuladns \
  -n nebuladns --create-namespace -f /tmp/values.yaml

# Smoke
helm test nebuladns -n nebuladns --logs

# Probe
kubectl -n nebuladns port-forward svc/nebuladns 15353:53 9090:9090 &
dig +tcp @127.0.0.1 -p 15353 www.example.com A +short
curl -s http://127.0.0.1:9090/metrics | head
```

For Docker + k3s end-to-end tests, the GitHub Actions workflows are already wired:

- [`.github/workflows/test-docker.yml`](.github/workflows/test-docker.yml)
- [`.github/workflows/test-k3s.yml`](.github/workflows/test-k3s.yml)

### systemd / Debian / Ubuntu

Hardened systemd unit with watchdog + `sd_notify`. Automated via Ansible.

```bash
# One-time host prep (on the target, as root):
adduser --disabled-password --gecos "NebulaDNS deploy" deploy
echo "<your SSH pub key>" >> /home/deploy/.ssh/authorized_keys
echo 'deploy ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/deploy-nebuladns

# On your machine (or GitHub runner):
cd deploy/ansible
ansible-galaxy collection install -r requirements.yml

# Copy inventory.example.ini → inventory.ini, edit hosts, then:
ansible-playbook -i inventory.ini playbook.yml \
  -e nebuladns_version=v0.1.0 \
  -e @environments/staging.yml

# Rollback in seconds (no downloads, symlink flip only):
ansible-playbook -i inventory.ini rollback.yml
```

See [`deploy/ansible/README.md`](deploy/ansible/README.md) for secrets, environments, and
the full release pipeline.

### Build from source

```bash
git clone https://github.com/nebuladns/nebuladns.git
cd nebuladns

cargo build --release --bin nebuladns --bin nebulactl

./target/release/nebuladns --config config/nebuladns.demo.toml &
dig @127.0.0.1 -p 15353 www.example.com A +short
./target/release/nebulactl health
```

Requires **Rust stable 1.80+** (pinned via `rust-toolchain.toml`).

---

## Why NebulaDNS

We ran TinyDNS (djbdns 1.05, released 2001) behind a CDN. In 22 days we had **two
production outages** on real customer traffic. Both traced to the same root cause:
djbdns 1.05 sends AXFR responses with `QDCOUNT=0`, which BIND 9.18+ rejects as FORMERR.
Our downstream secondaries silently upgraded to BIND 9.18, and our zone-transfer
redundancy eroded **invisibly for 13 months** until only one working agent remained.

There was no `/metrics`, no API, no post-deploy verification, no way to see that half
the fleet had stopped transferring. Engineers had to SSH into six servers to
reconstruct state during the incident.

We evaluated every serious authoritative DNS server in production use today.
Every single one has a structural limitation NebulaDNS is designed to overcome:

| Server | Critical limitation | NebulaDNS answer |
|--------|---------------------|------------------|
| **TinyDNS / djbdns 1.05** | No DNSSEC, no IXFR, no TSIG, `QDCOUNT=0` wire bug, no metrics, no API, unmaintained | Full RFC-conformant wire, modern Rust safety, API-first, `/metrics` native, TSIG/DNSSEC built in |
| **BIND 9** | Monolithic C with long CVE history, `named.conf` is file-only and fragile, `rndc` limited, `statistics-channels` is XML/JSON without Prometheus, no built-in UI | Single small binary, memory-safe Rust, `/metrics` native, REST + gRPC API, integrated UI, unified DNSSEC management |
| **Knot DNS** | No native REST API (control is `knotc` CLI), no built-in UI, no propagation verification | REST + gRPC + CLI + UI from day one, propagation verifier native |
| **NSD** | Intentionally minimal: no dynamic updates, no online DNSSEC signing, no API, no UI, no geo-routing | Full-featured while retaining NSD-class performance via Rust |
| **PowerDNS Auth** | API is incomplete (many operations still require SQL backend edits), multiple moving parts, no native propagation gate | API covers 100% of operations, propagation verifier built-in, single binary |
| **CoreDNS** | Not a full-fledged auth server for internet-facing zones, DNSSEC primitive, Go GC tail latency at high QPS | First-class authoritative + transfer + DNSSEC; drop-in replacement as cluster DNS; consistent low-tail latency (no GC) |
| **Route 53 / Cloudflare DNS** | Vendor lock-in, cost at scale, no self-hosted option | Self-hosted with the same UX quality; optionally integrates as a secondary |

The common pattern across every open-source alternative: **the control plane is an
afterthought bolted onto a 1990s-era data plane**. NebulaDNS inverts this — the API is
the server, and the data plane is a first-class implementation of the API's contract.

Most importantly: **no alternative ships a first-class propagation gate.** That single
primitive — "this change has reached all required downstreams; OK to proceed" — is the
feature that would have prevented both of our incidents. It's core to NebulaDNS from M6.

---

## Architecture

```
                       ┌──────────────────────────────────────────┐
                       │            React Web UI (v1.0)           │
                       │       Vite + TS + Tanstack Query         │
                       └───────────────────┬──────────────────────┘
                                           │ HTTPS (OpenAPI)
                       ┌───────────────────▼──────────────────────┐
                       │          Control Plane API               │
                       │   axum (REST) + tonic (gRPC)             │
                       │   AuthN: mTLS + OIDC   AuthZ: RBAC       │
                       └───────┬──────────────────┬───────────────┘
                               │                  │
                ┌──────────────▼──┐       ┌───────▼────────────┐
                │  Zone Manager   │       │ Propagation        │
                │  validate       │       │ Verifier           │
                │  sign (DNSSEC)  │       │  polls secondaries │
                │  atomic commit  │       │  confirms SOA      │
                └────────┬────────┘       └────────────────────┘
                         │
        ┌────────────────▼─────────────────────────┐
        │       Zone Store (content-addressed)     │
        │       sled / redb / pluggable            │
        └────────────────┬─────────────────────────┘
                         │
        ┌────────────────▼─────────────────────────┐
        │          DNS Data Plane                  │
        │   tokio + io_uring (Linux ≥ 5.19)        │
        │   UDP · TCP · DoT · DoH · DoQ            │
        │   AXFR / IXFR / NOTIFY / TSIG            │
        │   DNSSEC online signer                   │
        └──────────────────────────────────────────┘

    Observability spine: tracing → OpenTelemetry → Prometheus + Loki + Tempo
```

### Crate layout

| Crate | Purpose |
|-------|---------|
| `nebula-wire` | RFC 1035 wire codec. `#![forbid(unsafe_code)]`, fuzz-tested. |
| `nebula-proto` | High-level DNS message types (no I/O). |
| `nebula-zone` | TOML zone loader + indexed in-memory zone. |
| `nebula-signer` | DNSSEC online signing + key management (M4). |
| `nebula-store` | Storage abstraction + embedded backends (M5). |
| `nebula-transfer` | AXFR / IXFR / NOTIFY / TSIG (M3). |
| `nebula-server` | DNS data plane (tokio) + `nebuladns` binary. |
| `nebula-api` | REST (axum) + gRPC (tonic) + OpenAPI. |
| `nebula-verify` | Propagation verifier (M6). |
| `nebula-metrics` | Always-on Prometheus registry. |
| `nebula-cli` | `nebulactl` admin CLI. |
| `nebula-testutil` | Test harness + reference-peer drivers. |

---

## Observability

Metrics are **always on in production**. No "enable metrics" flag exists — there
is no scenario where observability competes with performance. Cardinality is bounded at
compile time via enum-typed labels.

```promql
# Wire / query pipeline — live today
nebula_dns_queries_total{proto,qtype,rcode}
nebula_dns_query_duration_seconds_bucket{proto,qtype,le}
nebula_dns_dropped_total{reason}

# Transfers (M3)
nebula_axfr_attempts_total{peer,zone,direction,result}
nebula_axfr_last_success_timestamp_seconds{peer,zone}
nebula_peer_version_info{peer,software,version}   # the 1326 signal

# Propagation verifier (M6)
nebula_zone_propagation_converged{zone}           # 0 | 1

# Runtime
nebula_build_info{version,commit,rustc,target} 1
nebula_process_resident_memory_bytes
nebula_memory_hot_path_allocations_total          # MUST stay at 0
```

A Grafana dashboard (`NebulaDNS Overview`) is provisioned automatically in the Docker
Compose rig at http://localhost:3000.

---

## Operations

- **systemd unit**: `deploy/systemd/nebuladns.service` — `Type=notify`, watchdog 30 s,
  `ProtectSystem=strict`, `AmbientCapabilities=CAP_NET_BIND_SERVICE`, seccomp filter,
  `MemoryDenyWriteExecute=true`.
- **Graceful shutdown**: SIGTERM/SIGINT drains connections, signals `STOPPING=1`, lets
  in-flight requests finish, then exits cleanly.
- **Rollback**: atomic binary symlink swap — MTTR ≈ seconds per host.
- **Zero-downtime restart**: supported via `nebuladns.socket` (socket activation).

---

## Security

- **Memory-safe by default**: `#![forbid(unsafe_code)]` in every crate. Exceptions
  require a written justification and a fuzz harness.
- **Continuous fuzzing**: 4 `cargo-fuzz` targets (header / question / name / message)
  run for 60 s on every PR; ClusterFuzzLite / OSS-Fuzz integration tracked for
  post-GA.
- **Supply chain**: `cargo deny` in CI with a whitelist of licenses
  (Apache-2.0, MIT, BSD-*, ISC, Unicode-*, CC0, Zlib, MPL-2.0).
  `cargo audit` for advisory database checks.
- **Signed artifacts (planned)**: SBOMs (CycloneDX + SPDX) + cosign keyless via GitHub
  OIDC + SLSA-3 provenance on every release.
- **Reporting**: see [`SECURITY.md`](SECURITY.md) — GPG key + 90-day coordinated
  disclosure.

---

## Repository layout

```
nebuladns/
├── crates/                    # 11 Cargo crates (see "Crate layout" above)
├── config/                    # example + compose + demo configs
├── zones/                     # example zone files
├── deploy/
│   ├── docker/                # Dockerfile + compose rig + Grafana dashboards
│   ├── helm/nebuladns/        # Helm chart (StatefulSet, Service, PDB, tests/)
│   ├── systemd/               # hardened .service + .socket + sysusers/tmpfiles
│   ├── ansible/               # role + playbook + environments + rollback
│   ├── prometheus/            # alerting rules (template in M6)
│   ├── grafana/               # upstream dashboards (template in M6)
│   └── terraform/             # Terraform module (placeholder)
├── docs/
│   ├── architecture.md        # one-page system view
│   ├── decisions/             # ADRs — toolchain, storage, consensus
│   └── runbooks/              # one per alert
├── ui/                        # React dashboard (M9)
├── xtask/                     # dev-loop tasks (`cargo xtask ci|fmt|clippy|fuzz|bench`)
├── .github/workflows/         # ci, deploy, test-docker, test-k3s
├── index.html                 # landing page (open directly in a browser)
├── PROJECT_PROMPT.md          # full planning prompt — the source of truth for intent
└── README.md                  # you are here
```

---

## Contributing

1. **Every feature ships with a test, a metric, and a doc section.** No exceptions.
2. **API-first**: every operator-visible behaviour must be an API call before any CLI
   or UI work merges.
3. **Every Prometheus alert has a runbook** in `docs/runbooks/`. CI enforces the link.
4. **Every non-obvious design choice needs an ADR** in `docs/decisions/`.

Dev loop:

```bash
cargo xtask ci        # fmt + clippy + tests + deny
cargo xtask fuzz wire # smoke-fuzz the wire codec
cargo xtask bench     # criterion benchmarks
```

Conventional Commits required (`feat(scope): …`, `fix(scope): …`). See
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the full guide.

---

## License

[Apache-2.0](LICENSE). See also [`SECURITY.md`](SECURITY.md),
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md), and
[`PROJECT_PROMPT.md`](PROJECT_PROMPT.md) (the planning document that serves as the
canonical source of design intent).

---

<p align="center">
  <em>Incidents 1273 and 1326 cannot recur in the same form. That's the bar.</em>
</p>
