//! Wire-parse errors.

use thiserror::Error;

/// Errors returned by the wire codec. Every variant is intentionally specific so that
/// operators and CI failures can reason about *what* went wrong on the wire, not just
/// that something did.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ParseError {
    /// Buffer was shorter than the 12-byte fixed header.
    #[error("message too short: need at least {HEADER_LEN} bytes for header, got {got}", HEADER_LEN = crate::HEADER_LEN)]
    TruncatedHeader { got: usize },

    /// Parser ran past the end of the buffer while consuming a field.
    #[error("unexpected end of buffer at offset {offset} while parsing {context}")]
    UnexpectedEof {
        offset: usize,
        context: &'static str,
    },

    /// Label exceeded RFC 1035 §2.3.4 63-byte limit.
    #[error("label length {len} exceeds maximum of {max}", max = crate::MAX_LABEL_LEN)]
    LabelTooLong { len: usize },

    /// Fully-qualified name exceeded RFC 1035 §2.3.4 255-byte limit.
    #[error("name length {len} exceeds maximum of {max}", max = crate::MAX_NAME_LEN)]
    NameTooLong { len: usize },

    /// Name compression pointer encountered in a context that forbids it (writer side),
    /// or pointer loop detected (reader side).
    #[error("invalid name compression: {0}")]
    InvalidCompression(&'static str),

    /// Reserved bits were set (RFC 1035 §4.1.1 `Z`). We treat these as a parse error to
    /// flag peers emitting non-standard traffic — this is exactly how we would have
    /// detected the djbdns QDCOUNT=0 anomaly earlier.
    #[error("reserved header bits (Z) were set: {bits:#b}")]
    ReservedBitsSet { bits: u8 },

    /// Unknown op-code.
    #[error("unknown opcode {0}")]
    UnknownOpCode(u8),

    /// Invalid QTYPE / TYPE value.
    #[error("invalid qtype {0}")]
    InvalidQType(u16),

    /// Invalid QCLASS / CLASS value.
    #[error("invalid qclass {0}")]
    InvalidQClass(u16),

    /// Provided buffer was too small to serialize the message.
    #[error("output buffer too small: need {need} bytes, have {have}")]
    OutputBufferTooSmall { need: usize, have: usize },

    /// QDCOUNT in the header doesn't match the number of question records parsed.
    /// The same class of defect that caused incidents 1273 / 1326.
    #[error("qdcount mismatch: header says {expected}, section has {got}")]
    QdCountMismatch { expected: u16, got: u16 },
}
