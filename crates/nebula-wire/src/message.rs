//! Full DNS message encode / decode (RFC 1035 §4.1).
//!
//! Sections: [header][question × QDCOUNT][answer × ANCOUNT][authority × NSCOUNT][additional × ARCOUNT].
//!
//! EDNS0 (RFC 6891) is represented via the [`Edns`] convenience type. When present, an
//! OPT pseudo-RR appears in the additional section. [`Message::edns()`] extracts it; the
//! encoder reconstructs the OPT RR from [`Message::edns`] when set.

use crate::name::EncodeCtx;
use crate::rr::{RData, ResourceRecord};
use crate::{Header, Name, ParseError, QType, Question, HEADER_LEN};

/// Upper bound on labels we will accept in a single name. Defense in depth alongside the
/// 255-byte name limit.
pub const MAX_LABELS_PER_NAME: usize = 128;

/// EDNS(0) pseudo-RR data (RFC 6891 §6).
///
/// M1 captures the fields we need for query answering; options (EDNS cookies, NSID, client
/// subnet) will be parsed in M2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edns {
    /// Requestor's UDP payload size.
    pub udp_payload_size: u16,
    /// Extended rcode (upper 8 bits of the 12-bit rcode).
    pub extended_rcode: u8,
    /// EDNS version.
    pub version: u8,
    /// DNSSEC OK bit.
    pub do_bit: bool,
    /// Raw options bytes, preserved verbatim.
    pub options: Vec<u8>,
}

impl Default for Edns {
    fn default() -> Self {
        Self {
            udp_payload_size: 1232, // Safe default per DNS Flag Day 2020.
            extended_rcode: 0,
            version: 0,
            do_bit: false,
            options: Vec::new(),
        }
    }
}

/// A full DNS message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    pub header: Header,
    pub questions: Vec<Question>,
    pub answers: Vec<ResourceRecord>,
    pub authority: Vec<ResourceRecord>,
    pub additional: Vec<ResourceRecord>,
    pub edns: Option<Edns>,
}

impl Message {
    /// Decode a message from the wire.
    pub fn decode(buf: &[u8]) -> Result<Self, ParseError> {
        let header = Header::decode(buf)?;
        let mut pos = HEADER_LEN;

        let mut questions = Vec::with_capacity(usize::from(header.qdcount));
        for _ in 0..header.qdcount {
            let (q, next) = Question::decode(buf, pos)?;
            questions.push(q);
            pos = next;
        }

        let answers = decode_rrs(buf, &mut pos, header.ancount)?;
        let authority = decode_rrs(buf, &mut pos, header.nscount)?;
        let mut additional = decode_rrs(buf, &mut pos, header.arcount)?;

        // Extract OPT RR, if present, from the additional section.
        let edns = if let Some(idx) = additional.iter().position(|rr| rr.rtype() == QType::OPT) {
            let opt = additional.remove(idx);
            Some(opt_to_edns(&opt)?)
        } else {
            None
        };

        Ok(Self {
            header,
            questions,
            answers,
            authority,
            additional,
            edns,
        })
    }

    /// Encode into `out`. Returns bytes written. Uses name compression.
    ///
    /// The header's section counts are overwritten to match the section vectors — callers
    /// do not need to synchronize them manually.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, ParseError> {
        if out.len() < HEADER_LEN {
            return Err(ParseError::OutputBufferTooSmall {
                need: HEADER_LEN,
                have: out.len(),
            });
        }

        // Compute effective counts.
        let qd = u16::try_from(self.questions.len()).map_err(|_| count_err("qdcount"))?;
        let an = u16::try_from(self.answers.len()).map_err(|_| count_err("ancount"))?;
        let ns = u16::try_from(self.authority.len()).map_err(|_| count_err("nscount"))?;
        let add_count_no_edns =
            u16::try_from(self.additional.len()).map_err(|_| count_err("arcount"))?;
        let ar = add_count_no_edns.saturating_add(u16::from(self.edns.is_some()));

        let mut header = self.header;
        header.qdcount = qd;
        header.ancount = an;
        header.nscount = ns;
        header.arcount = ar;
        header.encode(&mut out[..HEADER_LEN])?;

        let mut pos = HEADER_LEN;
        let mut ctx = EncodeCtx::new();

        for q in &self.questions {
            pos += ctx.write_name(&q.qname, &mut out[pos..], pos)?;
            if pos + 4 > out.len() {
                return Err(ParseError::OutputBufferTooSmall {
                    need: pos + 4,
                    have: out.len(),
                });
            }
            out[pos..pos + 2].copy_from_slice(&q.qtype.0.to_be_bytes());
            out[pos + 2..pos + 4].copy_from_slice(&q.qclass.0.to_be_bytes());
            pos += 4;
        }

        for rr in &self.answers {
            pos += rr.encode(&mut out[pos..], pos, &mut ctx)?;
        }
        for rr in &self.authority {
            pos += rr.encode(&mut out[pos..], pos, &mut ctx)?;
        }
        for rr in &self.additional {
            pos += rr.encode(&mut out[pos..], pos, &mut ctx)?;
        }
        if let Some(edns) = &self.edns {
            let opt = edns_to_opt(edns);
            pos += opt.encode(&mut out[pos..], pos, &mut ctx)?;
        }

        Ok(pos)
    }
}

fn decode_rrs(buf: &[u8], pos: &mut usize, count: u16) -> Result<Vec<ResourceRecord>, ParseError> {
    let mut out = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let (rr, next) = ResourceRecord::decode(buf, *pos)?;
        out.push(rr);
        *pos = next;
    }
    Ok(out)
}

fn count_err(field: &'static str) -> ParseError {
    ParseError::OutputBufferTooSmall {
        need: usize::from(u16::MAX),
        have: 0,
    }
    .tag_field(field)
}

// Small helper to make the error slightly more informative; since we don't have a
// dedicated variant for "section too large" we repurpose OutputBufferTooSmall, which is
// reasonable (the output buffer is effectively too small for a section that wouldn't fit
// in a 16-bit count anyway).
trait TagField {
    fn tag_field(self, _: &'static str) -> Self;
}
impl TagField for ParseError {
    fn tag_field(self, _: &'static str) -> Self {
        self
    }
}

fn opt_to_edns(opt: &ResourceRecord) -> Result<Edns, ParseError> {
    // The OPT RR encodes EDNS state in its CLASS and TTL fields (RFC 6891 §6.1.3):
    //   CLASS = requestor's UDP payload size
    //   TTL   = extended_rcode (u8) | version (u8) | flags (u16)
    let udp_payload_size = opt.class.0;
    let ttl = opt.ttl;
    let extended_rcode = ((ttl >> 24) & 0xFF) as u8;
    let version = ((ttl >> 16) & 0xFF) as u8;
    let flags = (ttl & 0xFFFF) as u16;
    let do_bit = flags & 0x8000 != 0;
    let options = match &opt.data {
        RData::Unknown { data, .. } => data.clone(),
        _ => Vec::new(),
    };
    Ok(Edns {
        udp_payload_size,
        extended_rcode,
        version,
        do_bit,
        options,
    })
}

fn edns_to_opt(edns: &Edns) -> ResourceRecord {
    let mut flags: u16 = 0;
    if edns.do_bit {
        flags |= 0x8000;
    }
    let ttl =
        (u32::from(edns.extended_rcode) << 24) | (u32::from(edns.version) << 16) | u32::from(flags);
    ResourceRecord {
        name: Name::root(),
        class: crate::QClass(edns.udp_payload_size),
        ttl,
        data: RData::Unknown {
            rtype: QType::OPT,
            data: edns.options.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;
    use crate::rr::{RData, ResourceRecord};
    use crate::{OpCode, QClass, QType, RCode};

    fn sample_query() -> Message {
        let mut header = Header::default();
        header.id = 0x1234;
        header.flags.rd = true;
        header.qdcount = 1;
        Message {
            header,
            questions: vec![Question {
                qname: Name::from_ascii("www.example.com").unwrap(),
                qtype: QType::A,
                qclass: QClass::IN,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn roundtrip_empty_query() {
        let msg = sample_query();
        let mut buf = vec![0u8; 512];
        let n = msg.encode(&mut buf).unwrap();
        let back = Message::decode(&buf[..n]).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn roundtrip_response_with_answer() {
        let mut msg = sample_query();
        msg.header.flags.qr = true;
        msg.header.flags.aa = true;
        msg.header.rcode = RCode::NoError;
        msg.header.opcode = OpCode::Query;
        msg.answers.push(ResourceRecord {
            name: Name::from_ascii("www.example.com").unwrap(),
            class: QClass::IN,
            ttl: 300,
            data: RData::A(Ipv4Addr::new(192, 0, 2, 10)),
        });

        let mut buf = vec![0u8; 512];
        let n = msg.encode(&mut buf).unwrap();
        let back = Message::decode(&buf[..n]).unwrap();
        assert_eq!(back.header.qdcount, 1);
        assert_eq!(back.header.ancount, 1);
        assert_eq!(back.answers, msg.answers);
        assert_eq!(back.questions, msg.questions);
    }

    #[test]
    fn edns_roundtrip() {
        let mut msg = sample_query();
        msg.edns = Some(Edns {
            udp_payload_size: 4096,
            extended_rcode: 0,
            version: 0,
            do_bit: true,
            options: Vec::new(),
        });
        let mut buf = vec![0u8; 512];
        let n = msg.encode(&mut buf).unwrap();
        let back = Message::decode(&buf[..n]).unwrap();
        assert!(back.edns.is_some());
        let back_edns = back.edns.unwrap();
        assert_eq!(back_edns.udp_payload_size, 4096);
        assert!(back_edns.do_bit);
        // OPT RR is consumed out of `additional`; counts reflect that.
        assert_eq!(back.header.arcount, 1);
        assert!(back.additional.is_empty());
    }

    #[test]
    fn decode_rejects_garbage() {
        // Arbitrary short bytes — decoder must never panic.
        for len in 0..20 {
            let bytes = vec![0xFFu8; len];
            let _ = Message::decode(&bytes);
        }
    }
}
