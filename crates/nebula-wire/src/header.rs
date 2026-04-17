//! DNS message header (RFC 1035 §4.1.1).
//!
//! ```text
//!                                 1  1  1  1  1  1
//!   0  1  2  3  4  5  6  7  8  9  0  1  2  3  4  5
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! |                      ID                       |
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! |QR|   Opcode  |AA|TC|RD|RA|   Z    |   RCODE   |
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! |                    QDCOUNT                    |
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! |                    ANCOUNT                    |
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! |                    NSCOUNT                    |
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! |                    ARCOUNT                    |
//! +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
//! ```

use crate::{ParseError, HEADER_LEN};

/// DNS opcode (RFC 1035 §4.1.1, RFC 2136, RFC 1996).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OpCode {
    /// Standard query.
    Query = 0,
    /// Inverse query (obsolete, RFC 3425).
    IQuery = 1,
    /// Server status request.
    Status = 2,
    /// NOTIFY (RFC 1996).
    Notify = 4,
    /// Dynamic update (RFC 2136).
    Update = 5,
}

impl OpCode {
    /// Parse a 4-bit opcode.
    pub fn from_u8(v: u8) -> Result<Self, ParseError> {
        match v {
            0 => Ok(Self::Query),
            1 => Ok(Self::IQuery),
            2 => Ok(Self::Status),
            4 => Ok(Self::Notify),
            5 => Ok(Self::Update),
            other => Err(ParseError::UnknownOpCode(other)),
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// DNS response code (RFC 1035 §4.1.1, RFC 6895).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RCode {
    NoError = 0,
    FormErr = 1,
    ServFail = 2,
    NxDomain = 3,
    NotImp = 4,
    Refused = 5,
    YxDomain = 6,
    YxRrSet = 7,
    NxRrSet = 8,
    NotAuth = 9,
    NotZone = 10,
}

impl RCode {
    /// Parse a 4-bit rcode. Unknown rcodes map to `ServFail` (RFC 6895 §2.3 guidance)
    /// rather than erroring — callers can still see the raw byte via `from_u8_raw`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::NoError,
            1 => Self::FormErr,
            2 => Self::ServFail,
            3 => Self::NxDomain,
            4 => Self::NotImp,
            5 => Self::Refused,
            6 => Self::YxDomain,
            7 => Self::YxRrSet,
            8 => Self::NxRrSet,
            9 => Self::NotAuth,
            10 => Self::NotZone,
            _ => Self::ServFail,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Header flag bits (QR, AA, TC, RD, RA, AD, CD).
///
/// RFC 4035 §4.6 / RFC 6840 §5.7 redefined two of the former Z bits:
/// AD (authenticated data) and CD (checking disabled) for DNSSEC. The remaining Z bit
/// must be zero on the wire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Flags {
    /// Query (false) / Response (true).
    pub qr: bool,
    /// Authoritative Answer.
    pub aa: bool,
    /// Truncated.
    pub tc: bool,
    /// Recursion Desired (copied from query in responses).
    pub rd: bool,
    /// Recursion Available.
    pub ra: bool,
    /// Authenticated Data (DNSSEC, RFC 4035 §4.6).
    pub ad: bool,
    /// Checking Disabled (DNSSEC, RFC 4035 §4.6).
    pub cd: bool,
}

/// The 12-byte DNS message header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub id: u16,
    pub flags: Flags,
    pub opcode: OpCode,
    pub rcode: RCode,
    pub qdcount: u16,
    pub ancount: u16,
    pub nscount: u16,
    pub arcount: u16,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            id: 0,
            flags: Flags::default(),
            opcode: OpCode::Query,
            rcode: RCode::NoError,
            qdcount: 0,
            ancount: 0,
            nscount: 0,
            arcount: 0,
        }
    }
}

impl Header {
    /// Decode a 12-byte DNS header from the start of `buf`.
    pub fn decode(buf: &[u8]) -> Result<Self, ParseError> {
        if buf.len() < HEADER_LEN {
            return Err(ParseError::TruncatedHeader { got: buf.len() });
        }
        let id = u16::from_be_bytes([buf[0], buf[1]]);
        let b2 = buf[2];
        let b3 = buf[3];

        let qr = (b2 & 0b1000_0000) != 0;
        let opcode_bits = (b2 & 0b0111_1000) >> 3;
        let aa = (b2 & 0b0000_0100) != 0;
        let tc = (b2 & 0b0000_0010) != 0;
        let rd = (b2 & 0b0000_0001) != 0;

        let ra = (b3 & 0b1000_0000) != 0;
        // Layout (b3): RA | Z | AD | CD | RCODE(4)
        let z = (b3 & 0b0100_0000) >> 6;
        let ad = (b3 & 0b0010_0000) != 0;
        let cd = (b3 & 0b0001_0000) != 0;
        let rcode_bits = b3 & 0b0000_1111;

        if z != 0 {
            return Err(ParseError::ReservedBitsSet { bits: z });
        }

        let opcode = OpCode::from_u8(opcode_bits)?;
        let rcode = RCode::from_u8(rcode_bits);

        let qdcount = u16::from_be_bytes([buf[4], buf[5]]);
        let ancount = u16::from_be_bytes([buf[6], buf[7]]);
        let nscount = u16::from_be_bytes([buf[8], buf[9]]);
        let arcount = u16::from_be_bytes([buf[10], buf[11]]);

        Ok(Self {
            id,
            flags: Flags {
                qr,
                aa,
                tc,
                rd,
                ra,
                ad,
                cd,
            },
            opcode,
            rcode,
            qdcount,
            ancount,
            nscount,
            arcount,
        })
    }

    /// Encode into `out`. Returns the number of bytes written.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, ParseError> {
        if out.len() < HEADER_LEN {
            return Err(ParseError::OutputBufferTooSmall {
                need: HEADER_LEN,
                have: out.len(),
            });
        }

        out[0..2].copy_from_slice(&self.id.to_be_bytes());

        let mut b2: u8 = 0;
        if self.flags.qr {
            b2 |= 0b1000_0000;
        }
        b2 |= (self.opcode.as_u8() & 0x0F) << 3;
        if self.flags.aa {
            b2 |= 0b0000_0100;
        }
        if self.flags.tc {
            b2 |= 0b0000_0010;
        }
        if self.flags.rd {
            b2 |= 0b0000_0001;
        }
        out[2] = b2;

        let mut b3: u8 = 0;
        if self.flags.ra {
            b3 |= 0b1000_0000;
        }
        // Z bit is zero by construction (we never emit it).
        if self.flags.ad {
            b3 |= 0b0010_0000;
        }
        if self.flags.cd {
            b3 |= 0b0001_0000;
        }
        b3 |= self.rcode.as_u8() & 0x0F;
        out[3] = b3;

        out[4..6].copy_from_slice(&self.qdcount.to_be_bytes());
        out[6..8].copy_from_slice(&self.ancount.to_be_bytes());
        out[8..10].copy_from_slice(&self.nscount.to_be_bytes());
        out[10..12].copy_from_slice(&self.arcount.to_be_bytes());

        Ok(HEADER_LEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_default_header() {
        let h = Header::default();
        let mut buf = [0u8; HEADER_LEN];
        let n = h.encode(&mut buf).unwrap();
        assert_eq!(n, HEADER_LEN);
        let decoded = Header::decode(&buf).unwrap();
        assert_eq!(h, decoded);
    }

    #[test]
    fn roundtrip_response_header() {
        let h = Header {
            id: 0xABCD,
            flags: Flags {
                qr: true,
                aa: true,
                tc: false,
                rd: true,
                ra: true,
                ad: false,
                cd: false,
            },
            opcode: OpCode::Query,
            rcode: RCode::NxDomain,
            qdcount: 1,
            ancount: 0,
            nscount: 1,
            arcount: 0,
        };
        let mut buf = [0u8; HEADER_LEN];
        h.encode(&mut buf).unwrap();
        let decoded = Header::decode(&buf).unwrap();
        assert_eq!(h, decoded);
    }

    #[test]
    fn truncated_header_rejected() {
        let buf = [0u8; 5];
        assert!(matches!(
            Header::decode(&buf),
            Err(ParseError::TruncatedHeader { got: 5 })
        ));
    }

    #[test]
    fn reserved_z_bit_rejected() {
        // Set the single remaining Z bit.
        let mut buf = [0u8; HEADER_LEN];
        buf[3] = 0b0100_0000;
        assert!(matches!(
            Header::decode(&buf),
            Err(ParseError::ReservedBitsSet { .. })
        ));
    }

    #[test]
    fn ad_and_cd_accepted_and_roundtrip() {
        // `dig` sets AD=1 on queries under DNSSEC-validator defaults.
        let h = Header {
            id: 1,
            flags: Flags {
                ad: true,
                cd: true,
                rd: true,
                ..Default::default()
            },
            opcode: OpCode::Query,
            rcode: RCode::NoError,
            qdcount: 1,
            ancount: 0,
            nscount: 0,
            arcount: 0,
        };
        let mut buf = [0u8; HEADER_LEN];
        h.encode(&mut buf).unwrap();
        let back = Header::decode(&buf).unwrap();
        assert_eq!(back, h);
        assert!(back.flags.ad && back.flags.cd);
    }

    #[test]
    fn notify_opcode_roundtrips() {
        let h = Header {
            opcode: OpCode::Notify,
            ..Header::default()
        };
        let mut buf = [0u8; HEADER_LEN];
        h.encode(&mut buf).unwrap();
        let decoded = Header::decode(&buf).unwrap();
        assert_eq!(decoded.opcode, OpCode::Notify);
    }

    #[test]
    fn unknown_opcode_rejected() {
        let mut buf = [0u8; HEADER_LEN];
        // opcode bits = 0b0110 (6) — unassigned.
        buf[2] = (6 & 0x0F) << 3;
        assert!(matches!(
            Header::decode(&buf),
            Err(ParseError::UnknownOpCode(6))
        ));
    }

    #[test]
    fn output_buffer_too_small_rejected() {
        let h = Header::default();
        let mut buf = [0u8; 5];
        assert!(matches!(
            h.encode(&mut buf),
            Err(ParseError::OutputBufferTooSmall { .. })
        ));
    }
}
