//! RFC 1035 DNS wire codec.
//!
//! This crate is the safety-critical core of NebulaDNS. It is `#![forbid(unsafe_code)]`,
//! fuzzed continuously, and parses strictly: every violation surfaces as a specific
//! [`ParseError`] variant so operators can reason about wire-level incompatibilities.
//!
//! **Why strict parsing matters**: djbdns 1.05 emits AXFR responses with `QDCOUNT=0`,
//! which BIND 9.18+ rejects as FORMERR. That silent incompatibility caused production
//! incidents 1273 and 1326. NebulaDNS will never emit a malformed header by construction,
//! and when it receives one it will report the exact mismatch — not a generic error.
//!
//! # Scope (M0)
//!
//! This module ships the RFC 1035 message header (§4.1.1) and question section (§4.1.2)
//! codec. Resource records, name compression, EDNS(0), and TSIG land in M1.

#![forbid(unsafe_code)]

pub mod error;
pub mod header;
pub mod message;
pub mod name;
pub mod question;
pub mod rr;

pub use error::ParseError;
pub use header::{Flags, Header, OpCode, RCode};
pub use message::{Edns, Message, MAX_LABELS_PER_NAME};
pub use name::{EncodeCtx, Name};
pub use question::{QClass, QType, Question};
pub use rr::{RData, ResourceRecord, Soa};

/// Maximum DNS message size over UDP without EDNS0 (RFC 1035 §4.2.1).
pub const MAX_UDP_MESSAGE_SIZE: usize = 512;

/// Maximum length of a DNS message on the wire. TCP prefixes with a 16-bit length.
pub const MAX_TCP_MESSAGE_SIZE: usize = 65_535;

/// Maximum length of a single DNS label (RFC 1035 §2.3.4).
pub const MAX_LABEL_LEN: usize = 63;

/// Maximum length of a fully-qualified domain name on the wire (RFC 1035 §2.3.4).
pub const MAX_NAME_LEN: usize = 255;

/// Size of the fixed DNS message header.
pub const HEADER_LEN: usize = 12;
