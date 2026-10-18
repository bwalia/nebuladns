# Runbook — NebulaDNS answer quality

Covers `NebulaDNSServfailRatioHigh`, `NebulaDNSRefusedRatioHigh`, `NebulaDNSLatencyHigh` and
`NebulaDNSDropsHigh`. Rules: `deploy/prometheus/nebuladns.rules.yml`.

The ratio and latency alerts only fire above 0.5 queries/s per instance, so a quiet DTAP
environment can't page on a handful of queries.

## Dashboards

`NebulaDNS Overview`: **Overview** stats, **Responses by rcode**, **Non-NOERROR responses**,
**Latency** row, **Drops by reason**. Filter with the `instance`, `qtype` and `rcode`
variables to isolate the source.

## SERVFAIL

`NebulaDNSServfailRatioHigh` (`page`): over 1% of answers have been SERVFAIL for 10 minutes.

**Impact:** an authoritative server should practically never SERVFAIL. Resolvers treat it
as a broken server, retry other NS records, and cache the failure briefly. Users see slow
or failed lookups for the affected names.

**Diagnosis:**
1. Which instance, and which query types? Filter the dashboard by `rcode=SERVFAIL`.
2. Reproduce: `dig @<instance-ip> <name> <type>` for a name from that zone.
3. Logs: `journalctl -u nebuladns -n 500 --no-pager | grep -i -E 'error|servfail'`.
4. Did it start with a deploy or zone change? Compare against the restart annotations on
   the dashboard and the deploy history.

**Remediation:**
- Started with a zone change: revert the zone file and `systemctl reload nebuladns`.
  Zone deploys validate with `--check-zone`, so a parse error can't get this far, but a
  valid-yet-wrong zone can.
- Started with a binary deploy: `ansible-playbook -i inventory.ini rollback.yml`.
- Only one instance affected: take it out of rotation (stop it; resolvers use the other
  NS) and investigate at leisure.

## REFUSED

`NebulaDNSRefusedRatioHigh` (`ticket`): over 25% of queries have been for zones this server
doesn't host, for 30 minutes.

**Impact:** none for hosted zones; NebulaDNS answers REFUSED and moves on. It means
something is pointed at the wrong server.

**Likely causes:**
- A client or host uses NebulaDNS as its **resolver** (`/etc/resolv.conf`, DHCP, a
  WireGuard peer's `DNS =` line). NebulaDNS is authoritative-only.
- A conditional forwarder is too broad: it forwards more than the private zone.
- Scanning: random names from many sources.

**Diagnosis:** on the host, see who's asking and for what:
```bash
sudo tcpdump -ni <dns-iface> -c 200 udp port 53
```
Then fix the client's resolver config or narrow the forwarder to the private zone only.

## Latency

`NebulaDNSLatencyHigh` (`ticket`): in-process p99 above 5 ms for 10 minutes. Normal is
~100 µs, and this measures time inside NebulaDNS, not the network.

**Likely causes:** CPU contention on the host (nginx or something else on the same box),
a very large RRset, or heavy TCP fallback.

**Diagnosis:** compare **p99 by instance** (one host or all?) and **p99 by query type**
(one type?). On the host: `top` / `pidstat -p $(pidof nebuladns) 5`. Check **Queries by
protocol**: a jump in TCP means answers are being truncated over UDP.

## Drops

`NebulaDNSDropsHigh` (`ticket`): over 10 requests/s dropped before a response for 10 minutes,
by `reason`.

- `malformed`: garbage packets, almost always scanners or fuzzers. Fine to ignore if the
  source is outside your network; otherwise find it with `tcpdump`.
- `rate_limited`, `policy`, `acl_deny`, `tsig_fail`, `overload`: reserved for features
  landing in M2/M3. If these appear, check the release notes for the running version.
