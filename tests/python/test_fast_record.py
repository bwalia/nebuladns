#!/usr/bin/env python3
"""Fast, authenticated CNAME/A change workflow.

Spawns `nebuladns` (or attaches to an already-running instance), proves that:

  1. Unauthenticated writes are refused.
  2. A CNAME can be published with TTL 5 and is visible on the data plane
     on the next UDP query — no reload, no restart.
  3. Retargeting the CNAME is equally immediate.
  4. Apex CNAME and CNAME-alongside-A are rejected (RFC 1034).
  5. The hash-chained audit log recorded the mutations.

Stdlib only (no dnspython). Usage:

    cargo build -p nebula-server --bin nebuladns
    python3 tests/python/test_fast_record.py --bin target/debug/nebuladns
"""

from __future__ import annotations

import argparse
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

QTYPE_A = 1
QTYPE_CNAME = 5
QCLASS_IN = 1


class Failed(Exception):
    pass


def encode_name(name: str) -> bytes:
    out = bytearray()
    for label in name.strip(".").split("."):
        raw = label.encode("ascii")
        out.append(len(raw))
        out.extend(raw)
    out.append(0)
    return bytes(out)


def read_name(buf: bytes, offset: int) -> tuple[str, int]:
    labels: list[str] = []
    jumped = False
    end = offset
    i = offset
    for _ in range(128):
        if i >= len(buf):
            raise Failed("truncated name")
        length = buf[i]
        if length == 0:
            i += 1
            return (".".join(labels) + "." if labels else "."), (end if jumped else i)
        if length & 0xC0 == 0xC0:
            if i + 1 >= len(buf):
                raise Failed("truncated pointer")
            ptr = ((length & 0x3F) << 8) | buf[i + 1]
            if not jumped:
                end = i + 2
            jumped = True
            i = ptr
            continue
        labels.append(buf[i + 1 : i + 1 + length].decode("ascii"))
        i += 1 + length
        if not jumped:
            end = i
    raise Failed("name loop")


def dns_query(addr: tuple[str, int], qname: str, qtype: int, timeout: float = 2.0):
    """Return (rcode, [(ttl, presentation, rtype), ...])."""
    header = struct.pack("!HHHHHH", 0xBEEF, 0x0000, 1, 0, 0, 0)
    question = encode_name(qname) + struct.pack("!HH", qtype, QCLASS_IN)
    payload = header + question
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.settimeout(timeout)
    try:
        sock.sendto(payload, addr)
        data, _ = sock.recvfrom(4096)
    finally:
        sock.close()
    if len(data) < 12:
        raise Failed(f"short DNS response ({len(data)} bytes)")
    _, flags, qd, an, _, _ = struct.unpack("!HHHHHH", data[:12])
    rcode = flags & 0xF
    pos = 12
    for _ in range(qd):
        _, pos = read_name(data, pos)
        pos += 4
    answers = []
    for _ in range(an):
        _, pos = read_name(data, pos)
        if pos + 10 > len(data):
            raise Failed("truncated RR")
        rtype, _, ttl, rdlen = struct.unpack("!HHIH", data[pos : pos + 10])
        rdata_at = pos + 10
        pos = rdata_at + rdlen
        if rtype == QTYPE_CNAME:
            presentation, _ = read_name(data, rdata_at)
        elif rtype == QTYPE_A and rdlen == 4:
            presentation = socket.inet_ntoa(data[rdata_at : rdata_at + 4])
        else:
            presentation = data[rdata_at:pos].hex()
        answers.append((ttl, presentation, rtype))
    return rcode, answers


def http_json(method: str, url: str, token: str | None, body=None, timeout: float = 3.0):
    data = None if body is None else json.dumps(body).encode()
    headers = {"Accept": "application/json"}
    if data is not None:
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
            parsed = json.loads(raw.decode() or "null")
            return resp.status, parsed
    except urllib.error.HTTPError as exc:
        raw = exc.read()
        try:
            parsed = json.loads(raw.decode() or "null")
        except json.JSONDecodeError:
            parsed = {"error": raw.decode(errors="replace")}
        return exc.code, parsed


def free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


ZONE_TOML = """origin = "example.com."
default_ttl = 300

[soa]
mname = "ns1.example.com."
rname = "hostmaster.example.com."
serial = 1
refresh = 10800
retry = 3600
expire = 604800
minimum = 300

[[records]]
name = "@"
type = "NS"
value = "ns1.example.com."

[[records]]
name = "www"
type = "A"
value = "192.0.2.10"
"""


def wait_live(url: str, attempts: int = 50) -> None:
    for _ in range(attempts):
        try:
            status, _ = http_json("GET", url, None)
            if status == 200:
                return
        except OSError:
            time.sleep(0.05)
        else:
            time.sleep(0.05)
    raise Failed(f"server never became live at {url}")


def expect(cond: bool, msg: str) -> None:
    if not cond:
        raise Failed(msg)


def run_against(api: str, dns: tuple[str, int], token: str) -> None:
    records = f"{api}/api/v1/zones/example.com/records"
    audit = f"{api}/api/v1/audit"

    status, body = http_json(
        "PUT",
        records,
        None,
        {"name": "app", "type": "CNAME", "value": "west.example.net."},
    )
    expect(status in (401, 403), f"unauthenticated write should fail, got {status} {body}")

    status, body = http_json(
        "PUT",
        records,
        "wrong-token",
        {"name": "app", "type": "CNAME", "value": "west.example.net."},
    )
    expect(status == 401, f"wrong token should be 401, got {status} {body}")

    status, body = http_json(
        "PUT",
        records,
        token,
        {"name": "app", "type": "CNAME", "value": "west.example.net.", "ttl": 5},
    )
    expect(status == 200, f"CNAME upsert failed: {status} {body}")
    expect(body["records"][0]["ttl"] == 5, f"expected ttl 5, got {body}")
    expect(
        body["records"][0]["value"] == "west.example.net.",
        f"unexpected CNAME target {body}",
    )

    rcode, answers = dns_query(dns, "app.example.com", QTYPE_CNAME)
    expect(rcode == 0, f"DNS rcode {rcode} after upsert")
    expect(answers, "no CNAME answers on the wire")
    ttl, target, rtype = answers[0]
    expect(rtype == QTYPE_CNAME, f"expected CNAME, got type {rtype}")
    expect(ttl == 5, f"wire TTL {ttl}, expected 5")
    expect(target == "west.example.net.", f"wire target {target!r}")

    status, body = http_json(
        "PUT",
        records,
        token,
        {"name": "app", "type": "CNAME", "value": "east.example.net.", "ttl": 5},
    )
    expect(status == 200, f"CNAME retarget failed: {status} {body}")
    _, answers = dns_query(dns, "app.example.com", QTYPE_CNAME)
    target = answers[0][1]
    expect(target == "east.example.net.", f"retarget not visible, still {target!r}")

    status, body = http_json(
        "PUT",
        records,
        token,
        {"name": "@", "type": "CNAME", "value": "elsewhere.example.net."},
    )
    expect(status == 400, f"apex CNAME should be 400, got {status} {body}")

    status, body = http_json(
        "PUT",
        records,
        token,
        {"name": "www", "type": "CNAME", "value": "cdn.example.net."},
    )
    expect(status == 400, f"CNAME+A coexistence should be 400, got {status} {body}")

    status, body = http_json(
        "PUT",
        f"{records}?dry_run=true",
        token,
        {"name": "tmp", "type": "A", "value": "192.0.2.99"},
    )
    expect(status == 200 and body["dry_run"] is True, f"dry_run failed {status} {body}")
    rcode, answers = dns_query(dns, "tmp.example.com", QTYPE_A)
    expect(rcode == 3 or not answers, "dry_run leaked a record onto the wire")

    status, body = http_json("GET", audit, token)
    expect(status == 200, f"audit log failed {status} {body}")
    events = body["events"]
    expect(len(events) >= 2, f"expected audit events, got {events}")
    expect(events[1]["prev_hash"] == events[0]["hash"], "audit chain broken")
    print("ok: fast CNAME change, low TTL, auth, RFC checks, audit chain")


def spawn_and_run(binary: Path) -> None:
    api_port = free_port()
    dns_port = free_port()
    token = "python-workflow-token"
    tmp = tempfile.TemporaryDirectory(prefix="nebuladns-fast-")
    root = Path(tmp.name)
    zone_path = root / "example.com.toml"
    zone_path.write_text(ZONE_TOML)
    cfg_path = root / "nebuladns.toml"
    cfg_path.write_text(
        f"""
[api]
bind = "127.0.0.1:{api_port}"

[metrics]
bind = "127.0.0.1:{free_port()}"

[logging]
filter = "warn"
json = false

[dns]
udp = "127.0.0.1:{dns_port}"
tcp = "127.0.0.1:{dns_port}"

[[zones]]
file = "{zone_path}"
"""
    )
    env = os.environ.copy()
    env["NEBULA_API_TOKEN"] = token
    proc = subprocess.Popen(
        [str(binary), "--config", str(cfg_path)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    try:
        wait_live(f"http://127.0.0.1:{api_port}/livez")
        run_against(f"http://127.0.0.1:{api_port}", ("127.0.0.1", dns_port), token)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        tmp.cleanup()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", help="Path to nebuladns binary (spawn mode)")
    parser.add_argument("--api", help="Existing admin API base URL")
    parser.add_argument("--dns-host", default="127.0.0.1")
    parser.add_argument("--dns-port", type=int, default=15353)
    parser.add_argument("--token", default=os.environ.get("NEBULA_API_TOKEN", ""))
    args = parser.parse_args()
    try:
        if args.api:
            if not args.token:
                raise Failed("--token or NEBULA_API_TOKEN required with --api")
            run_against(args.api.rstrip("/"), (args.dns_host, args.dns_port), args.token)
        elif args.bin:
            spawn_and_run(Path(args.bin))
        else:
            default = Path("target/debug/nebuladns")
            if not default.exists():
                raise Failed("pass --bin or cargo build -p nebula-server --bin nebuladns")
            spawn_and_run(default)
    except Failed as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
