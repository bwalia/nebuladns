# Runbook — DnsFailover

## Summary

`nebula-dns-failover` probes edge POP `/healthz` endpoints and rewrites **managed**
DNS records so a hostname fails over from primary (lon1) to secondary (lon2) when
the primary stays down, then fails back when healthy again.

DNS-only: clients discover the new IP after **TTL + recursive cache** expiry.
Expect RTO roughly `2–3 × TTL` (TTL 30s → ~60–90s under typical resolvers).

## Severity

`page` when `nebula_dns_failover_state` stays `degraded_both_down` or apply errors spike.

## Impact

Hostname may keep pointing at a dead edge until TTL expires after a successful
failover apply. If both POPs are down the controller **fail-opens** (keeps
last-known-good DNS) and alerts — it never blanks the RRset.

## Architecture

Pick **one** provider. NebulaDNS and Cloudflare are alternatives, not stacked.

| Mode | Zone nameservers | Need NebulaDNS host? | Token |
|------|------------------|----------------------|-------|
| `provider.type = "nebuladns"` | e.g. `ns1.nebuladns.net` (+ `ns2…` for secondary) | **Yes** — NebulaDNS serves `:53` and the record API | `NEBULA_API_TOKEN` |
| `provider.type = "cloudflare"` | Cloudflare NS | **No** for failover writes | `CF_API_TOKEN` |

**Do not confuse:**

| Name | Layer |
|------|--------|
| `ns1` / `ns2.nebuladns.net` | Authoritative **DNS servers** for the zone (NebulaDNS primary / secondary). |
| `lon1` / `lon2` | Edge **traffic** POPs. Failover points application A records at these IPs. |

`nebula-dns-failover` always needs a **control host**. That is separate from hosting NebulaDNS (only required in NebulaDNS mode).

Full diagrams: [`crates/nebula-dns-failover/README.md`](../../crates/nebula-dns-failover/README.md#system-architecture).

## Where to run

Run on a **control host that is not the POP under test** (or accept SPOF if the
controller shares fate with secondary). Do not rely solely on in-POP probes.

## Config & secrets

| Secret / path | Purpose |
|---------------|---------|
| `/etc/nebuladns/dns-failover.toml` | Hostnames, POPs, provider, hysteresis |
| `NEBULA_API_TOKEN` | Bearer for NebulaDNS `PUT /api/v1/zones/.../records` (fail-closed) |
| `CF_API_TOKEN` | Cloudflare Zone → DNS → Edit (only if `provider.type = "cloudflare"`) |
| `FAILOVER_API_TOKEN` | Bearer for `POST /v1/hostnames/...` break-glass |

A hostname should be managed by **either** static multi-A provisioning **or** this
controller, not both. Cloudflare writes carry comment marker
`nebula-dns-failover | hostname=… | policy=… | v=1`. Unmarked records are never
deleted. NebulaDNS ownership is config membership of the FQDN.

## Dashboards / signals

- `GET http://127.0.0.1:9119/v1/status`
- `GET http://127.0.0.1:9119/metrics`
  - `nebula_dns_failover_pop_up{pop=…}`
  - `nebula_dns_failover_state{hostname=…}`
  - `nebula_dns_failover_transitions_total`
  - `nebula_dns_failover_dns_apply_total{result=…}`
  - `nebula_dns_failover_last_success_unixtime`

## Diagnosis

```bash
curl -s localhost:9119/v1/status | jq .
curl -s localhost:9119/metrics | grep nebula_dns_failover
# dry-run once against live health endpoints
nebula-dns-failover --config /etc/nebuladns/dns-failover.toml --once --dry-run
echo $?   # 0 ok, 2 config/auth, 3 apply failed, 4 both down (fail-open)
```

## Manual failover / failback

```bash
export TOK=…   # FAILOVER_API_TOKEN
# Force secondary
curl -s -X POST -H "Authorization: Bearer $TOK" \
  localhost:9119/v1/hostnames/abtesting.fictionally.org/failover
# Force primary
curl -s -X POST -H "Authorization: Bearer $TOK" \
  localhost:9119/v1/hostnames/abtesting.fictionally.org/failback
# Return to automatic (health-driven) — overrides are sticky until cleared
curl -s -X POST -H "Authorization: Bearer $TOK" \
  localhost:9119/v1/hostnames/abtesting.fictionally.org/auto
# Reconcile now
curl -s -X POST -H "Authorization: Bearer $TOK" \
  localhost:9119/v1/hostnames/abtesting.fictionally.org/reconcile
```

## Pilot cutover — `abtesting.fictionally.org`

1. Inventory current record (historically CNAME → `lon2.pop0.uk` on Cloudflare).
2. Prefer a test name first: `abtesting-failover-test.fictionally.org`.
3. Enable controller with `dry_run = true`; confirm `/v1/status` POP health.
4. Apply once (`dry_run = false`); replace CNAME with managed A(s) to POP `public_ipv4`.
5. Verify HTTPS still works; dig `@1.1.1.1` / `@8.8.8.8` after TTL.
6. Enable systemd unit; document break-glass endpoints above.
7. Optional chaos: controlled lon1 drain during a maintenance window.

## Remediation

- Flapping: raise `consecutive_fail` / `consecutive_ok` or set `failback_delay`.
- Wrong target: check `dry_run`, provider auth, and that unmarked CF records were not
  expected to move.
- Both down: restore at least one POP `/healthz`; DNS stays on last good until then.

## Related

- Crate README + flow diagrams: [`crates/nebula-dns-failover/README.md`](../../crates/nebula-dns-failover/README.md)
- ADR [`0002-fast-record-api.md`](../decisions/0002-fast-record-api.md)
- Binary: `crates/nebula-dns-failover`
- Deploy: `deploy/dns-failover/`
