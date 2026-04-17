# Runbook — `NebulaDNSDown`

## Summary

The Prometheus scraper cannot reach `/metrics` on a NebulaDNS instance for 2+ minutes.

## Severity

`page`

## Impact

A NebulaDNS pod is unreachable. If this is the only replica, DNS is down and resolvers
are serving stale or no data.

## Dashboards

- `NebulaDNS Overview` Grafana dashboard
- `NebulaDNS Runtime & Cost`

## Diagnosis

1. Check pod status:
   ```
   kubectl -n <ns> get pods -l app.kubernetes.io/name=nebuladns
   kubectl -n <ns> describe pod <pod>
   kubectl -n <ns> logs <pod> --tail=200
   ```
2. Check liveness from inside the cluster:
   ```
   kubectl -n <ns> port-forward <pod> 8080:8080
   curl -sv http://localhost:8080/livez
   ```
3. If the container is crash-looping, inspect the JSON logs for the fatal record.

## Remediation

- Pod OOM-killed → raise `resources.limits.memory` in `values.yaml`; re-roll.
- CrashLoopBackOff with config error → `kubectl get cm -o yaml` and validate the mounted
  `nebuladns.toml`; server prints the specific field that failed to parse.
- Node cordon / AZ outage → `PodDisruptionBudget` should prevent total loss; confirm
  another replica is serving. If none, a manual `kubectl rollout restart` on a different
  node pool may be needed.
- If the whole `StatefulSet` is down, fall back to the static secondary declared in the
  parent zone while you investigate.

## Escalation

- Tier-1 on-call: 15 minutes, then page tier-2.
- Hardware / network causes: loop in platform team.
- If the root cause appears to be a wire-level incompatibility (FORMERR / truncated), this
  is the pattern from incident 1326 — escalate immediately and capture a packet capture.
