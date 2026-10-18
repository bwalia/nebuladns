# Runbook — DnsFailover

## Summary

`nebula-dns-failover` probes edge POP `/healthz` endpoints and rewrites **managed**
DNS records so a hostname fails over from primary (lon1) to secondary (lon2) when
the primary stays down, then fails back when healthy again.

DNS-only: clients discover the new IP after **TTL + recursive cache** expiry.
Expect RTO roughly `2–3 × TTL` (TTL 30s → ~60–90s under typical resolvers).

## Severity

Alerts (rules in `deploy/prometheus/nebuladns.rules.yml`, sections below):

| Alert | Severity |
|---|---|
| `NebulaDNSFailoverControllerDown` | `page` |
| `NebulaDNSFailoverApplyFailing` | `page` |
| `NebulaDNSFailoverBothPopsDown` | `page` |
| `NebulaDNSFailoverPopDown` | `ticket` |
| `NebulaDNSFailoverFlapping` | `ticket` |
| `NebulaDNSFailoverManualOverrideStale` | `ticket` |

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

- `NebulaDNS Overview` → **Failover controller** row: serving state, POP health, state
  changes, DNS apply results, transition history.
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

## Alert: NebulaDNSFailoverControllerDown

Prometheus can't scrape the controller for 2 minutes. **Nothing is watching POP health**, so
a POP outage now will not move DNS. Records stay where they are, which is fine until a POP
fails.

```bash
systemctl status nebula-dns-failover
journalctl -u nebula-dns-failover -n 200 --no-pager
curl -s localhost:9119/v1/status | jq .
```
Exit codes in the logs: `2` config/auth, `3` apply failed, `4` both down. Fix and restart.
If it won't start and a POP is down right now, fail over by hand at the provider.

## Alert: NebulaDNSFailoverApplyFailing

The controller has decided on a target but **every DNS write fails** (5+ minutes), so the
record may still point at a dead POP. The serving-state panel shows the *decision*, not
what DNS serves; check the record itself:
```bash
dig @<authoritative-ns> <hostname> A +short
journalctl -u nebula-dns-failover -n 50 --no-pager | grep -i 'apply failed'
```
- `401` / `403`: token wrong or rotated (`NEBULA_API_TOKEN` / `CF_API_TOKEN`).
- `404`: wrong `zone_name`, or `api_base` points at the wrong process or port.
- `409 Idempotency-Key reused`: the pre-#97 controller bug. Upgrade the controller.
- Connection refused: the NebulaDNS admin API is down. See `NebulaDNSDown`.

If it can't be fixed quickly, set the record by hand at the provider, then put a manual
override on the hostname so the controller doesn't fight you.

## Alert: NebulaDNSFailoverBothPopsDown

Serving state `degraded` (4): neither POP passes health checks. The controller
**fail-opens**: DNS stays on the last good target and is never blanked.
1. Is it really both POPs, or the controller's network? Probe the health URLs from another
   host.
2. If only the controller's view is broken, fix its network. DNS is untouched meanwhile.
3. Otherwise restore at least one POP; the controller fails over or back on its own.

## Alert: NebulaDNSFailoverPopDown

One POP has failed health checks for 5 minutes (`ticket`). Hostnames with it as primary
should already be on the secondary: confirm on the serving-state panel and with `dig`.
Fix the POP; failback is automatic after `consecutive_ok` good probes (plus
`failback_delay`, if set).

## Alert: NebulaDNSFailoverFlapping

More than 4 serving-state changes in 30 minutes. Every flip waits out resolver caches
(about 2–3 × TTL), so flapping is worse than staying put. Raise `consecutive_fail` /
`consecutive_ok`, add `failback_delay`, or pin the hostname with a manual override while
the POP's health is investigated.

## Alert: NebulaDNSFailoverManualOverrideStale

A hostname has been in `manual_primary` / `manual_secondary` for over 4 hours. Overrides
are sticky, so health-driven failover is **off** for it. If the reason is gone, return it to
automatic:
```bash
curl -s -X POST -H "Authorization: Bearer $TOK" localhost:9119/v1/hostnames/<hostname>/auto
```

## Related

- Crate README + flow diagrams: [`crates/nebula-dns-failover/README.md`](../../crates/nebula-dns-failover/README.md)
- ADR [`0002-fast-record-api.md`](../decisions/0002-fast-record-api.md)
- Binary: `crates/nebula-dns-failover`
- Deploy: `deploy/dns-failover/`
