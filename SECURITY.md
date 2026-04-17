# Security Policy

## Reporting a vulnerability

Please report security vulnerabilities privately to `security@nebuladns.io` (GPG key fingerprint
will be published at GA). **Do not open a public GitHub issue for security reports.**

We follow a 90-day coordinated-disclosure policy. You will receive:

- An acknowledgement within 48 hours
- A preliminary assessment within 7 days
- A fix or mitigation plan within the SLO below

## Fix SLO

| Severity | Fix SLO |
|----------|---------|
| Critical | 48 hours |
| High     | 7 days  |
| Medium   | 30 days |
| Low      | Best effort, next release |

## Supported versions

Once v1.0 ships, three concurrent release lines will be supported for 18 months each, with
every other `v1.x` designated an LTS with 3-year security patches. The current status:

| Version | Status       | End of support |
|---------|--------------|----------------|
| 0.1.x (M0) | Pre-release (no support) | — |

## Supply-chain integrity

- All releases are signed with cosign (keyless, GitHub OIDC)
- SBOMs (CycloneDX + SPDX) are attached to every release
- SLSA level 3 provenance attestations published alongside artifacts
- Reproducible builds documented in `docs/reproducible-builds.md`

## Secure by default

- `#![forbid(unsafe_code)]` in every crate that does not genuinely require `unsafe`
- Continuous fuzzing on the wire codec, zone parser, and DNSSEC signer
- EDNS cookies, per-client rate limits, and RRL enabled by default
- DNSSEC private keys encrypted at rest; HSM/KMS supported
- Append-only, hash-chained audit log
