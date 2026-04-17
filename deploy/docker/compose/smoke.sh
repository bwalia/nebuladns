#!/bin/sh
# Smoke test: dig a known zone against NebulaDNS and assert the answers. Exits non-zero
# on any failure so `docker compose run smoke` is a usable CI gate.
#
# Usage: smoke.sh <host> <port>

set -eu

HOST="${1:-nebuladns}"
PORT="${2:-53}"

# Install dig + curl (alpine minimal).
apk add --no-cache bind-tools curl >/dev/null

pass=0
fail=0

check() {
  desc="$1"; shift
  expected="$1"; shift
  actual="$(dig +short +time=3 +tries=2 "$@" @"$HOST" -p "$PORT" 2>&1 | sort | tr '\n' ' ' | sed 's/ *$//')"
  wanted="$(printf '%s' "$expected" | sort | tr '\n' ' ' | sed 's/ *$//')"
  if [ "$actual" = "$wanted" ]; then
    echo "  pass  $desc"
    pass=$((pass + 1))
  else
    echo "  FAIL  $desc"
    echo "    expected: $wanted"
    echo "    actual:   $actual"
    fail=$((fail + 1))
  fi
}

echo "== UDP =="
check "A www.example.com"     "192.0.2.10 192.0.2.11" www.example.com A
check "AAAA www.example.com"  "2001:db8::10"          www.example.com AAAA
check "NS example.com"        "ns1.example.com. ns2.example.com." example.com NS
check "MX example.com"        "10 mail.example.com."  example.com MX

echo "== TCP =="
check "A www.example.com (TCP)"     "192.0.2.10 192.0.2.11" +tcp www.example.com A

echo "== status codes =="
nxdomain_status="$(dig @"$HOST" -p "$PORT" nope.example.com A 2>/dev/null | awk '/status:/ {print $6}' | tr -d ',')"
refused_status="$(dig @"$HOST" -p "$PORT" foo.not-our-zone.test A 2>/dev/null | awk '/status:/ {print $6}' | tr -d ',')"
if [ "$nxdomain_status" = "NXDOMAIN" ]; then
  echo "  pass  NXDOMAIN for nope.example.com"
  pass=$((pass + 1))
else
  echo "  FAIL  NXDOMAIN expected, got: $nxdomain_status"
  fail=$((fail + 1))
fi
if [ "$refused_status" = "REFUSED" ]; then
  echo "  pass  REFUSED for out-of-zone"
  pass=$((pass + 1))
else
  echo "  FAIL  REFUSED expected, got: $refused_status"
  fail=$((fail + 1))
fi

echo "== /metrics =="
metrics_body="$(curl -fsS --max-time 3 "http://$HOST:9090/metrics" 2>&1 || echo MISSING)"
if printf '%s' "$metrics_body" | grep -q '^nebula_dns_queries_total'; then
  echo "  pass  nebula_dns_queries_total exposed"
  pass=$((pass + 1))
else
  echo "  FAIL  metrics endpoint did not expose nebula_dns_queries_total"
  fail=$((fail + 1))
fi

echo "=============================="
echo "  passed: $pass"
echo "  failed: $fail"
if [ "$fail" -gt 0 ]; then
  exit 1
fi
