//! DNS question section (RFC 1035 §4.1.2).

use crate::{Name, ParseError};

/// DNS QTYPE (§3.2.2 TYPE superset in §3.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QType(pub u16);

impl QType {
    pub const A: Self = Self(1);
    pub const NS: Self = Self(2);
    pub const CNAME: Self = Self(5);
    pub const SOA: Self = Self(6);
    pub const PTR: Self = Self(12);
    pub const MX: Self = Self(15);
    pub const TXT: Self = Self(16);
    pub const AAAA: Self = Self(28);
    pub const SRV: Self = Self(33);
    pub const OPT: Self = Self(41);
    pub const DS: Self = Self(43);
    pub const RRSIG: Self = Self(46);
    pub const NSEC: Self = Self(47);
    pub const DNSKEY: Self = Self(48);
    pub const NSEC3: Self = Self(50);
    pub const CAA: Self = Self(257);
    pub const AXFR: Self = Self(252);
    pub const IXFR: Self = Self(251);
    pub const ANY: Self = Self(255);
}

/// DNS QCLASS (§3.2.4 + §3.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QClass(pub u16);

impl QClass {
    pub const IN: Self = Self(1);
    pub const CH: Self = Self(3);
    pub const HS: Self = Self(4);
    pub const ANY: Self = Self(255);
}

/// A single DNS question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub qname: Name,
    pub qtype: QType,
    pub qclass: QClass,
}

impl Question {
    /// Wire size in bytes.
    #[must_use]
    pub fn wire_len(&self) -> usize {
        self.qname.wire_len() + 4
    }

    /// Encode into `out`. Returns bytes written.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, ParseError> {
        let need = self.wire_len();
        if out.len() < need {
            return Err(ParseError::OutputBufferTooSmall {
                need,
                have: out.len(),
            });
        }
        let name_bytes = self.qname.encode(out)?;
        let tail = &mut out[name_bytes..need];
        tail[0..2].copy_from_slice(&self.qtype.0.to_be_bytes());
        tail[2..4].copy_from_slice(&self.qclass.0.to_be_bytes());
        Ok(need)
    }

    /// Decode from `buf` starting at `offset`. Returns the question and the offset just
    /// past the last consumed byte.
    pub fn decode(buf: &[u8], offset: usize) -> Result<(Self, usize), ParseError> {
        let (qname, pos) = Name::decode(buf, offset)?;
        if pos + 4 > buf.len() {
            return Err(ParseError::UnexpectedEof {
                offset: pos,
                context: "question tail",
            });
        }
        let qtype = QType(u16::from_be_bytes([buf[pos], buf[pos + 1]]));
        let qclass = QClass(u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]));
        Ok((
            Self {
                qname,
                qtype,
                qclass,
            },
            pos + 4,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_a_question() {
        let q = Question {
            qname: Name::from_ascii("www.example.com").unwrap(),
            qtype: QType::A,
            qclass: QClass::IN,
        };
        let mut buf = vec![0u8; q.wire_len()];
        q.encode(&mut buf).unwrap();
        let (decoded, end) = Question::decode(&buf, 0).unwrap();
        assert_eq!(decoded, q);
        assert_eq!(end, buf.len());
    }

    #[test]
    fn roundtrip_root_axfr_question() {
        let q = Question {
            qname: Name::root(),
            qtype: QType::AXFR,
            qclass: QClass::IN,
        };
        let mut buf = vec![0u8; q.wire_len()];
        q.encode(&mut buf).unwrap();
        let (decoded, _) = Question::decode(&buf, 0).unwrap();
        assert_eq!(decoded, q);
    }

    #[test]
    fn truncated_qtype_tail_rejected() {
        // Encode just the name with no type/class bytes.
        let n = Name::from_ascii("example.com").unwrap();
        let mut buf = vec![0u8; n.wire_len()];
        n.encode(&mut buf).unwrap();
        assert!(matches!(
            Question::decode(&buf, 0),
            Err(ParseError::UnexpectedEof { .. })
        ));
    }
}
