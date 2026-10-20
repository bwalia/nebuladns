---
name: dns-level-failover
description: >-
  Add DNS-level health failover for a test domain (CNAME or A) between edge POPs
  lon1/lon2 using NebulaDNS ns1/ns2 or Cloudflare, low TTL, nebula-dns-failover,
  and an optional demo page that shows the failover path. Use when the user asks
  for DNS failover, POP failover, lon1/lon2 switchover, abtesting.fictionally.org
  style failover, test.failover.nebuladns.net, or a page that shows TTL and which
  POP served the request.
disable-model-invocation: false
---

# DNS-level failover for a test domain

Teach any frontier model to stand up **health-driven DNS failover**: when primary
edge POP `lon1` fails `/healthz`, rewrite a low-TTL CNAME/A to secondary `lon2`
(and fail back when lon1 recovers). Reference pilot:
[https://abtesting.fictionally.org/](https://abtesting.fictionally.org/). Live
NebulaDNS demo hostname: `test.failover.nebuladns.net`.

## Non-negotiables

- **ns1/ns2 ≠ lon1/lon2.** Nameservers vs HTTP edge POPs.
- Pick **one** write provider: NebulaDNS **or** Cloudflare — never both.
- Run `nebula-dns-failover` on a control path that can reach the record API;
  for NebulaDNS with localhost-only admin API, run the controller **on each NS**.
- Failover is DNS-only: clients move after **~2–3 × TTL** (TTL 30 → ~60–90s).
- Demo page must be deployed on **both** POPs so a CNAME flip still serves HTML.

## Architecture (choose one)

### A — NebulaDNS authoritative (ns1/ns2) — preferred for product demos

```
User → resolver → ns1/ns2.nebuladns.net (:53)
                     ↑ PUT CNAME via record API
              nebula-dns-failover (on each NS)
                     ↓ probe
              lon1 / lon2 /healthz
```

1. Child zone e.g. `failover.example.com` with NS → `ns1`/`ns2`.
2. Hostname e.g. `test.failover.example.com` CNAME → `lon1.pop0.uk.` TTL **30**.
3. Controller: `provider.type = "nebuladns"`, `api_base` local admin API,
   `record_type = "CNAME"`, `cname_targets = { lon1 = "lon1.pop0.uk.", lon2 = "lon2.pop0.uk." }`.

### B — Cloudflare authoritative — preferred for production app names

Same controller; `provider.type = "cloudflare"` + `CF_API_TOKEN`. Apex stays on
Cloudflare NS. Example cutover name: `abtesting.fictionally.org` (see
`docs/runbooks/DnsFailover.md`).

## Checklist (NebulaDNS path)

Copy this and fill in.

```text
[ ] POPs: lon1 IPv4 + https://lon1…/healthz ; lon2 IPv4 + /healthz both 200
[ ] Zone TOML on ns1 and ns2 (Ansible-synced); apex NS + hostname CNAME TTL 30
[ ] Parent DNS: NS delegation for the child zone to ns1/ns2 (glue A for ns hosts)
[ ] NEBULA_API_TOKEN on nebuladns + dns-failover env (fail-closed record API)
[ ] nebula-dns-failover.toml: interval 10s, consecutive_fail/ok 3, ttl 30, dry_run false
[ ] Binary + systemd unit on BOTH NS hosts; curl :9119/v1/status shows pops + hostname
[ ] Demo page + /pop.json on BOTH edge POPs for the Host header
[ ] Verify: dig @ns1 FQDN CNAME +ttl ; dig @ns2 … ; open https://FQDN/
[ ] Chaos: stop lon1 edge → after ~30s CNAME→lon2 → page shows lon2 → restore lon1
```

## Config skeleton

```toml
listen = "127.0.0.1:9119"
dry_run = false
interval = "10s"
probe_timeout = "3s"
consecutive_fail = 3
consecutive_ok = 3
failback_delay = "30s"
ttl = 30

[provider]
type = "nebuladns"   # or "cloudflare"
api_base = "http://127.0.0.1:18080"
zone_name = "failover.example.com"

[pops.lon1]
public_ipv4 = "195.20.255.201"
health_urls = ["https://lon1.pop0.uk/healthz"]

[pops.lon2]
public_ipv4 = "85.190.106.189"
health_urls = ["https://lon2.pop0.uk/healthz"]

[[hostnames]]
name = "test.failover.example.com"
mode = "active-passive"
primary = "lon1"
secondary = "lon2"
record_type = "CNAME"
cname_targets = { lon1 = "lon1.pop0.uk.", lon2 = "lon2.pop0.uk." }
enabled = true
```

## Demo page (failover path + TTL)

Ship `deploy/failover-demo/index.html` to both POPs’ web roots for the demo Host.
Also ship `pop.json` per POP (`deploy/failover-demo/pop.lon1.json` → `/pop.json`).

The page must show:

1. **You landed on** — POP id from `/pop.json` (proves lon1-down → visited via lon2).
2. **DNS says now** — DoH CNAME/A + **TTL**.
3. **lon1/lon2 /healthz** — live probe status.
4. **Path chips** — User → NS → lon1/lon2 with the active POP highlighted.
5. **Story** — healthy vs failover vs failback narrative.

This estate’s live demo: `test.failover.nebuladns.net` (zone + controller in
sibling `diy-tax-return-infra`: `ansible/nebuladns-failover.yml`,
`scripts/nebuladns-cloudflare-dns.py`).

## Break-glass

```bash
# On an NS host (token in /etc/nebuladns/dns-failover.env)
curl -s -X POST -H "Authorization: Bearer $FAILOVER_API_TOKEN" \
  http://127.0.0.1:9119/v1/hostnames/test.failover.example.com/failover  # → lon2
curl -s -X POST -H "Authorization: Bearer $FAILOVER_API_TOKEN" \
  http://127.0.0.1:9119/v1/hostnames/test.failover.example.com/failback  # → lon1
curl -s -X POST -H "Authorization: Bearer $FAILOVER_API_TOKEN" \
  http://127.0.0.1:9119/v1/hostnames/test.failover.example.com/auto
```

## Verify commands

```bash
dig @72.62.211.28 test.failover.nebuladns.net CNAME +ttl
dig @85.190.106.88 test.failover.nebuladns.net CNAME +ttl
curl -sS https://lon1.pop0.uk/healthz; curl -sS https://lon2.pop0.uk/healthz
curl -sS http://127.0.0.1:9119/v1/status   # on ns1 or ns2
open https://test.failover.nebuladns.net/  # or abtesting.fictionally.org
```

## This repo pointers

| Piece | Path |
|-------|------|
| Failover crate / design | `crates/nebula-dns-failover/README.md` |
| Operator runbook | `docs/runbooks/DnsFailover.md` |
| Example config | `deploy/dns-failover/config.example.toml` |
| Demo UI | `deploy/failover-demo/index.html` |
| NS + Cloudflare glue docs | `docs/runbooks/NsSetupAndCloudflare.md` |
| Infra Ansible (deploy) | sibling `diy-tax-return-infra/ansible/nebuladns*.yml` |

## Do not

- Stack NebulaDNS + Cloudflare writers for the same hostname.
- Use TTL > 60 for a failover test (slow demo).
- Run the only controller solely on lon1 (fate-shares with the POP under test).
- Forget `/pop.json` on lon2 — failover “works” in dig but the page cannot prove the path.
