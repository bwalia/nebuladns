# Cloudflare NS ops for a new host

Read this when editing parent-zone DNS to delegate a name to ns1/ns2.

## Token

```bash
export KUBECONFIG=~/.kube/k3s1.yaml
# Or: CF_API_TOKEN / CF_TOKEN_FILE
```

Script: `diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py`

## ZONE_OPS pattern (new hostname)

Copy the `fictionally.org` style block. Replace zone + name.

```python
{
    "zone": "example.com",
    "delete": [
        {"types": ["CNAME", "A", "AAAA"], "name": "app.example.com"},
    ],
    "upsert": [
        {"type": "NS", "name": "app.example.com", "content": "ns1.nebuladns.net", "ttl": 300},
        {"type": "NS", "name": "app.example.com", "content": "ns2.nebuladns.net", "ttl": 300},
    ],
}
```

For a **child zone** under `nebuladns.net` (e.g. `failover.nebuladns.net`), usually
only NS upserts are needed (no delete) once glue A for ns1/ns2 exist:

```python
{"type": "NS", "name": "failover.nebuladns.net", "content": "ns1.nebuladns.net", "ttl": 300},
{"type": "NS", "name": "failover.nebuladns.net", "content": "ns2.nebuladns.net", "ttl": 300},
```

Glue (once per estate, DNS-only / `proxied: False`):

```python
{"type": "A", "name": "ns1.nebuladns.net", "content": "72.62.211.28", "proxied": False, "ttl": 300},
{"type": "A", "name": "ns2.nebuladns.net", "content": "85.190.106.88", "proxied": False, "ttl": 300},
```

## Apply

```bash
python3 diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py --dry-run
python3 diy-tax-return-infra/scripts/nebuladns-cloudflare-dns.py
```

## Verify parent + authority

```bash
dig NS app.example.com +short
dig @72.62.211.28 app.example.com SOA +norecurse
dig @85.190.106.88 app.example.com SOA +norecurse
```

Both NS IPs must answer; public recursion may lag by parent NS TTL.
