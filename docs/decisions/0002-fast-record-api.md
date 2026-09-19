# ADR 0002 — Authenticated fast record API with fail-closed token auth

- **Status**: Accepted
- **Date**: 2026-09-16
- **Deciders**: @nebuladns/maintainers
- **Supersedes**: —

## Context

Operators need to retarget CNAME (and A/AAAA/TXT) records in seconds during
failover, with a low TTL so resolver caches expire quickly. The control plane
had no mutation API (M5 was still planned). A write path that is merely fast
is an incident amplifier if it is unauthenticated: anyone who can reach the
admin bind can hijack a zone.

Security of DNS changes is the constraint, not a follow-up.

## Decision

1. **Fail closed.** `/api/v1/zones*` and `/api/v1/audit` refuse every request
   unless a bearer token is configured (`NEBULA_API_TOKEN` and/or
   `api.token_sha256`) *and* the request presents a matching token.
   `/livez`, `/readyz`, and `/api/v1/version` stay unauthenticated.
2. **Store a digest, never the secret.** Comparison is SHA-256 of the presented
   token vs the configured digest, using constant-time equality.
3. **Atomic publish.** Mutations clone the zone, apply RFC checks, bump the SOA
   serial, and `arc-swap` a new snapshot. In-flight DNS queries see the old or
   the new RRset, never a torn one.
4. **Low TTL by default.** Writes use TTL 5 unless specified; the server
   rejects TTLs outside `min_ttl..=max_ttl` (default 1..=60) so a caller cannot
   accidentally pin a failover target in caches for hours.
5. **RFC-facing guards.** Apex CNAME is rejected. CNAME cannot coexist with any
   other type at the same owner. SOA and apex NS cannot be mutated through this
   path. Owners outside the origin are rejected.
6. **Audit.** Every mutation (including dry-run) is appended to a hash-chained
   in-memory audit log served at `GET /api/v1/audit`.
7. **Idempotency + dry-run.** `Idempotency-Key` replays the original response
   without a second serial bump. `?dry_run=true` validates without publishing.

## Rationale

Loopback bind is not authentication. Helm and docker-compose already bind the
admin API on `0.0.0.0`. Fail-closed writes mean a forgotten token cannot become
a silent open relay. Constant-time digest compare closes a timing oracle on
the token. CNAME rules encode RFC 1034 §3.6.2 in the API so a fast failover
script cannot produce an un-servable zone.

## Consequences

- Demo / CI must set `NEBULA_API_TOKEN` to exercise the record API. Health and
  `dig` smoke tests are unchanged.
- mTLS and OIDC (PROJECT_PROMPT §5) remain the M5 destination; the token is the
  break-glass / machine credential that section already named.
- Full zone replace, history, and rollback stay M5.

## Alternatives considered

- **Open writes on loopback.** Rejected: compose/helm expose `0.0.0.0:8080`.
- **Wait for M5.** Rejected: failover needs the write path now; the security
  controls above are a subset of the M5 contract, not a shortcut around it.
- **TTL 0 default.** Rejected: some resolvers treat 0 as "do not cache this
  response at all" and re-query in a tight loop. 5s is fast enough for
  failover and polite to resolvers. Operators can set `min_ttl = 0`.
