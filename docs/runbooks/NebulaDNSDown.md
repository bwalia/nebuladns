# Runbook — `NebulaDNSDown`

Also covers `NebulaDNSAllInstancesDown`, `NebulaDNSTargetsMissing` and
`NebulaDNSVersionDrift` (sections below). Rules: `deploy/prometheus/nebuladns.rules.yml`.

## Summary

Prometheus has not scraped `/metrics` on a NebulaDNS instance for 2+ minutes.

## Severity

`page`

## Impact

One authoritative server is unreachable. Resolvers retry the other NS records, so with
two or more healthy instances users see slower first lookups, not failures. If this was
the only instance, see [All instances down](#all-instances-down).

## Dashboards

`NebulaDNS Overview` → **Fleet** row (Instances table, Scrape health) and
**Queries by instance**.

## Diagnosis

A scrape failure is either the process being down or Prometheus not reaching it.

**systemd hosts (Ansible deploy):**
```bash
systemctl status nebuladns
journalctl -u nebuladns -n 200 --no-pager        # JSON logs; look for the fatal record
curl -s http://127.0.0.1:8080/livez               # admin API; 127.0.0.1:18080 with environments/private.yml
curl -s http://127.0.0.1:9090/metrics | head      # metrics bind (19090 with private.yml)
dig @<dns-bind-ip> <zone> SOA +short              # is DNS itself answering?
```
If DNS answers but `/metrics` doesn't, it's a scrape problem: check the metrics bind
address, the host firewall, and the Prometheus target address.

**Kubernetes:**
```bash
kubectl -n <ns> get pods -l app.kubernetes.io/name=nebuladns
kubectl -n <ns> describe pod <pod>
kubectl -n <ns> logs <pod> --tail=200
kubectl -n <ns> port-forward <pod> 8080:8080 && curl -sv http://localhost:8080/livez
```

## Remediation

- **Crashed / exited:** `journalctl` (or pod logs) shows the cause. A config or zone parse
  error names the file and field. Fix it and `systemctl restart nebuladns`, or roll back:
  `ansible-playbook -i inventory.ini rollback.yml`.
- **Bind failure (`EADDRINUSE`):** something else holds :53. On Ubuntu/Debian that's
  usually the systemd-resolved stub; bind NebulaDNS to a specific IP
  (`nebuladns_dns_interface`) instead of `0.0.0.0`.
- **WireGuard bind not available at boot:** the unit is ordered after `wg-quick@<iface>`;
  check `systemctl status wg-quick@wg0` and the drop-in
  `/etc/systemd/system/nebuladns.service.d/10-dns-interface.conf`.
- **Pod OOM-killed:** raise `resources.limits.memory` in `values.yaml`; re-roll.
- `systemctl reload nebuladns` re-reads zones (SIGHUP) and is safe; it no longer kills
  the process.

## All instances down

`NebulaDNSAllInstancesDown`: nothing in the job is scrapeable. **DNS for the hosted zones
fails once resolver caches expire** (SOA minimum / record TTLs).

1. Confirm from outside Prometheus: `dig @<each-ns-ip> <zone> SOA`.
2. If DNS is actually answering, Prometheus lost reachability (network change, firewall,
   Prometheus itself). Treat as [Targets missing](#targets-missing).
3. If DNS is down everywhere, restore one instance first (restart, or `rollback.yml` if a
   deploy just went out), then the rest.

## Targets missing

`NebulaDNSTargetsMissing`: Prometheus has **no** series for `up{job=~".*nebuladns.*"}`, so
`NebulaDNSDown` can't fire. Usually a renamed scrape job (the rules expect `nebuladns` in
the job name), a removed ServiceMonitor, or broken service discovery. Check
Prometheus → Status → Targets.

## Version drift

`NebulaDNSVersionDrift` (`ticket`): instances have reported different `version` labels on
`nebula_build_info` for over an hour, so a rollout or rollback stopped part-way. The Fleet
table shows which instance runs what. Finish the rollout (re-run the deploy) or roll the
stragglers back.

## Escalation

- Tier-1 on-call: 15 minutes, then page tier-2.
- Hardware / network causes: loop in platform team.
- If the root cause appears to be a wire-level incompatibility (FORMERR / truncated), this
  is the pattern from incident 1326 — escalate immediately and capture a packet capture.
