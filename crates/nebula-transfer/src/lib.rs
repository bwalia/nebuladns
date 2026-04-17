//! Zone transfer protocols.
//!
//! AXFR (RFC 5936), IXFR (RFC 1995), NOTIFY (RFC 1996), TSIG (RFC 8945).
//!
//! Strict conformance: AXFR responses will carry `QDCOUNT=1` (the djbdns
//! `QDCOUNT=0` bug that caused incident 1326 is impossible here by construction).

#![forbid(unsafe_code)]

pub const MARKER: &str = "nebula-transfer";
