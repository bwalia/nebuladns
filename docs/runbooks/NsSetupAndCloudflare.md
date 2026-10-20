# Runbook — ns1/ns2 setup and Cloudflare

## Summary

Publish `ns1.nebuladns.net` / `ns2.nebuladns.net`, install NebulaDNS on those
hosts, and choose how Cloudflare relates to the zone for DNS failover.

**How to use this DNS in incidents** (queries, triage table, failover vs NS):
see the main [README section](../../README.md#using-this-dns-ns1--ns2-for-incidents-and-failover)
and the sibling infra runbook `diy-tax-return-infra/docs/runbooks/nebuladns.md`.

Operational Ansible + SSH live in **diy-tax-return-infra**
(`ansible/nebuladns.yml`, `docs/runbooks/nebuladns.md`). This file is the
product-side companion.

## Hosts

| Name | Inventory | IP |
|------|-----------|-----|
| ns1 | cloud001 | `72.62.211.28` |
| ns2 | vps002 | `85.190.106.88` |

## Cloudflare steps (split model — default)

Apex `nebuladns.net` already uses Cloudflare NS (`mitch` / `fiona`).

**Automated (preferred):** sibling infra script, using the same API token as
wslproxy DnsManager on k3s1:

```bash
export KUBECONFIG=~/.kube/k3s1.yaml
python3 diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py
```

Token source: `kubectl -n wslproxy-system get secret wslproxy-settings` →
`settings.json` → `dns.providers[0].api_token` (or `CF_API_TOKEN` /
`CF_TOKEN_FILE`).

Creates DNS-only **A** `ns1`/`ns2` and **NS** `demo` → ns1 + ns2. Leave the
zone on Cloudflare nameservers.

## Failover options

| Mode | Zone NS | Failover provider | Best when |
|------|---------|-------------------|-----------|
| **C Split (recommended)** | Cloudflare for apex; Nebula for delegated children | `cloudflare` | Production today |
| **B Cloudflare only** | Cloudflare | `cloudflare` | No need for public Nebula NS |
| **A Nebula apex** | Custom NS → ns1/ns2 | `nebuladns` | Productizing self-hosted auth DNS |

**Best for DNS failover today:** Mode C or B with
`nebula-dns-failover` + `provider.type = "cloudflare"`. Cloudflare anycast
covers NS redundancy; the controller rewrites managed A/CNAMEs. Do not stack
NebulaDNS and Cloudflare as write providers.

See also: [DnsFailover.md](./DnsFailover.md).

## Option A cutover (later)

1. Deploy a full `nebuladns.net` zone to both NS (Ansible-synced).
2. Confirm `dig @72.62.211.28 nebuladns.net SOA` and same on ns2.
3. Cloudflare → Custom nameservers → `ns1.nebuladns.net` /
   `ns2.nebuladns.net` with glue.
4. Switch failover config to `provider.type = "nebuladns"`.

Until AXFR/IXFR ships, treat dual-NS dynamic API mutations as primary-only;
prefer Ansible zone sync for static records.
