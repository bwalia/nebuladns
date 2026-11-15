---
name: add-host-ns-failover
description: >-
  Add NebulaDNS ns1/ns2 nameserver delegation for a hostname or child zone, then
  point that name at health-driven DNS failover (lon1↔lon2 via nebula-dns-failover).
  Use whenever the user asks to put a host on ns1/ns2, delegate a zone to
  NebulaDNS, change a domain’s DNS to DNS failover, wire failover.nebuladns.net /
  abtesting.fictionally.org style NS cutover, or “add ns1 and ns2 for this host”.
  Also use for Cloudflare glue A records for ns1/ns2 and NS upserts that replace
  an existing CNAME/A.
compatibility: Works with Claude Code, Cursor, and any agent that loads Agent Skills (SKILL.md). Needs shell, dig/curl, and repo/sibling-infra access — no vendor-specific tools required.
---

# Add ns1/ns2 for a host → DNS failover

Portable Agent Skill: teach **any** coding agent to (1) delegate a hostname or
child zone to NebulaDNS **ns1** + **ns2**, then (2) manage that name with
**nebula-dns-failover** so traffic flips lon1 → lon2 on `/healthz` failure.

Live references: `test.failover.nebuladns.net`, `abtesting.fictionally.org`.
Estate IPs and deeper controller detail: read sibling skill
`dns-level-failover` and `docs/runbooks/NsSetupAndCloudflare.md` /
`docs/runbooks/DnsFailover.md` when needed.

## Non-negotiables

- **ns1/ns2 ≠ lon1/lon2.** ns\* = authoritative DNS hosts. lon\* = HTTP edge POPs.
- Pick **one** write provider for the failover controller: `nebuladns` **or**
  `cloudflare` — never both for the same hostname.
- Before publishing **NS** at a name, delete conflicting **CNAME/A/AAAA** at that
  exact owner (NS and CNAME cannot coexist at the same name).
- Failover moves clients after **~2–3 × TTL** (prefer TTL **30** for tests).
- Run the controller on a path that can reach the record API; for NebulaDNS with
  localhost-only admin API, run it **on each NS host**.

## Fleet constants

| Role | Name | IP |
|------|------|-----|
| ns1 | `ns1.nebuladns.net` | `72.62.211.28` (cloud001) |
| ns2 | `ns2.nebuladns.net` | `85.190.106.88` (vps002) |
| lon1 | edge POP | `195.20.255.201` — `https://lon1.pop0.uk/healthz` |
| lon2 | edge POP | `85.190.106.189` — `https://lon2.pop0.uk/healthz` |

## Collect inputs first

Ask only for what is missing:

1. **FQDN** to put on failover (e.g. `app.example.com` or `test.failover.example.com`)
2. **Parent zone** that currently holds public DNS (often Cloudflare)
3. **Mode** (default = recommended):
   - **N — Nebula NS child** (preferred when user says “add ns1/ns2”): parent
     publishes `NS` → ns1/ns2; NebulaDNS serves the zone; failover
     `provider.type = "nebuladns"`
   - **C — Cloudflare writer** (preferred for production app names that stay on
     Cloudflare NS): skip NS cutover; only add the hostname to the controller with
     `provider.type = "cloudflare"`
4. **Record shape**: `CNAME` to `lon1.pop0.uk.` / `lon2.pop0.uk.` **or** `A` to POP
   `public_ipv4`
5. Confirm both POP `/healthz` return 200 before enabling live writes

## Workflow N — Nebula NS + failover (default for “add ns1/ns2”)

Copy and tick:

```text
[ ] 1. Glue: A ns1/ns2.nebuladns.net exist (DNS-only / not proxied)
[ ] 2. Parent: delete CNAME/A/AAAA at the delegated name if present
[ ] 3. Parent: upsert NS name → ns1.nebuladns.net + ns2.nebuladns.net
[ ] 4. Zone TOML on BOTH ns1 and ns2 (Ansible-synced); SOA + hostname RRset TTL 30
[ ] 5. dig @72.62.211.28 and @85.190.106.88 show SOA / target record
[ ] 6. nebula-dns-failover.toml: provider nebuladns, hostname, pops, ttl 30
[ ] 7. Controller + token on BOTH NS; dry_run then live; /v1/status OK
[ ] 8. Public dig + HTTPS; optional chaos (stop lon1 → record flips → restore)
```

### Step details

**1–3. Parent DNS (Cloudflare typical)**

Prefer the sibling infra script (same CF token as wslproxy DnsManager):

```bash
export KUBECONFIG=~/.kube/k3s1.yaml
# Preview then apply — edit ZONE_OPS in the script for a new hostname first
python3 diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py --dry-run
python3 diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py
```

For a **new** host, extend `ZONE_OPS` like the `fictionally.org` / `abtesting`
block: `delete` CNAME/A/AAAA at the name, then `upsert` two NS RRs to
`ns1.nebuladns.net` and `ns2.nebuladns.net`. Pattern and API helpers:
[references/cloudflare-ns-ops.md](references/cloudflare-ns-ops.md).

**4. Zone on NebulaDNS**

Ship zone TOML to both NS (via `diy-tax-return-infra` Ansible / nebuladns playbooks).
Minimum RRset: apex `SOA` + `NS`, plus the failover hostname as `CNAME` or `A`
with **TTL 30**. Reload/restart so both IPs answer.

**5. Ground-truth dig**

```bash
FQDN=test.failover.example.com   # replace
dig @72.62.211.28 $FQDN CNAME +ttl +norecurse
dig @85.190.106.88 $FQDN CNAME +ttl +norecurse
dig @72.62.211.28 $(echo $FQDN | sed 's/^[^.]*\.//') SOA +short
```

**6–7. Failover controller**

Use `deploy/dns-failover/config.example.toml` as the skeleton. For Nebula NS mode:

```toml
[provider]
type = "nebuladns"
api_base = "http://127.0.0.1:18080"   # localhost admin on each NS
zone_name = "failover.example.com"

[[hostnames]]
name = "test.failover.example.com"
mode = "active-passive"
primary = "lon1"
secondary = "lon2"
record_type = "CNAME"
cname_targets = { lon1 = "lon1.pop0.uk.", lon2 = "lon2.pop0.uk." }
enabled = true
```

Secrets: `NEBULA_API_TOKEN` (record API), `FAILOVER_API_TOKEN` (break-glass).
Start with `dry_run = true`, confirm `/v1/status`, then set `dry_run = false`.
Unit example: `deploy/dns-failover/nebula-dns-failover.service`.

**8. Verify + break-glass**

```bash
curl -sS https://lon1.pop0.uk/healthz; curl -sS https://lon2.pop0.uk/healthz
curl -sS http://127.0.0.1:9119/v1/status   # on ns1 or ns2
# Force secondary / primary / auto — see DnsFailover.md
```

## Workflow C — Cloudflare NS, failover writer only

When the user wants DNS failover **without** moving public NS:

```text
[ ] Keep apex on Cloudflare NS
[ ] Ensure ns1/ns2 glue only if you still want Nebula for other delegated zones
[ ] Add hostname to nebula-dns-failover with provider.type = "cloudflare"
[ ] CF_API_TOKEN with Zone DNS Edit; dry_run → live; dig @1.1.1.1 after TTL
```

Do **not** also publish Nebula as writer for that same FQDN.

## Mode picker (say this to the user if unclear)

| Goal | Mode | Failover `provider.type` |
|------|------|---------------------------|
| “Put this host on ns1/ns2” / product Nebula demo | **N** | `nebuladns` |
| Production app name, stay on Cloudflare anycast | **C** | `cloudflare` |
| Full apex cutover to Nebula (rare) | Option A in NsSetup runbook | `nebuladns` after registrar NS change |

## Do not

- Stack NebulaDNS + Cloudflare writers on one hostname
- Leave a CNAME at the same owner as new NS records
- Use TTL > 60 for a failover demo
- Run the only controller solely on lon1 (fate-shares with the POP under test)
- Confuse “add ns1/ns2” (delegation) with “flip lon1/lon2” (record rewrite)

## Repo pointers

| Piece | Path |
|-------|------|
| This skill (canonical) | `.cursor/skills/add-host-ns-failover/` |
| Failover demo / controller skill | `.cursor/skills/dns-level-failover/` |
| CF glue + NS script | sibling `diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py` |
| NS + CF runbook | `docs/runbooks/NsSetupAndCloudflare.md` |
| Failover runbook | `docs/runbooks/DnsFailover.md` |
| Example config | `deploy/dns-failover/config.example.toml` |
| CF NS op patterns | [references/cloudflare-ns-ops.md](references/cloudflare-ns-ops.md) |
