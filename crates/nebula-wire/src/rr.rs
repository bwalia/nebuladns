//! Resource records (RFC 1035 §3.2 / §4.1.3).
//!
//! RR wire format:
//! ```text
//! +------------------+
//! |      NAME        |  (compressed, §4.1.4)
//! +------------------+
//! |      TYPE        |  (u16)
//! +------------------+
//! |     CLASS        |  (u16)
//! +------------------+
//! |      TTL         |  (u32, signed per RFC 2181 — treat as unsigned)
//! +------------------+
//! |    RDLENGTH      |  (u16)
//! +------------------+
//! |     RDATA        |  (variable)
//! +------------------+
//! ```
//!
//! M1 supports the common record types. The wire format is version-agnostic — every RR
//! flows through one encoder / decoder; per-type handling is only for RDATA.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::name::EncodeCtx;
use crate::{Name, ParseError, QClass, QType};

/// A DNS resource record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRecord {
    pub name: Name,
    pub class: QClass,
    pub ttl: u32,
    pub data: RData,
}

impl ResourceRecord {
    /// Convenience: derive the `TYPE` from the RDATA variant.
    #[must_use]
    pub fn rtype(&self) -> QType {
        self.data.rtype()
    }
}

/// RDATA — the per-type data portion of a resource record.
///
/// `Unknown` preserves unsupported types verbatim so we can still transfer zones that
/// include RR types we haven't implemented yet. This is essential for being a *good*
/// secondary — we must forward bytes we don't understand rather than rejecting them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RData {
    A(Ipv4Addr),
    Aaaa(Ipv6Addr),
    Ns(Name),
    Cname(Name),
    Ptr(Name),
    Mx {
        preference: u16,
        exchange: Name,
    },
    Txt(Vec<Vec<u8>>),
    Soa(Soa),
    Srv {
        priority: u16,
        weight: u16,
        port: u16,
        target: Name,
    },
    Caa {
        flags: u8,
        tag: Vec<u8>,
        value: Vec<u8>,
    },
    Unknown {
        rtype: QType,
        data: Vec<u8>,
    },
}

impl RData {
    #[must_use]
    pub fn rtype(&self) -> QType {
        match self {
            Self::A(_) => QType::A,
            Self::Aaaa(_) => QType::AAAA,
            Self::Ns(_) => QType::NS,
            Self::Cname(_) => QType::CNAME,
            Self::Ptr(_) => QType::PTR,
            Self::Mx { .. } => QType::MX,
            Self::Txt(_) => QType::TXT,
            Self::Soa(_) => QType::SOA,
            Self::Srv { .. } => QType::SRV,
            Self::Caa { .. } => QType::CAA,
            Self::Unknown { rtype, .. } => *rtype,
        }
    }
}

/// SOA RDATA (RFC 1035 §3.3.13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Soa {
    pub mname: Name,
    pub rname: Name,
    pub serial: u32,
    pub refresh: u32,
    pub retry: u32,
    pub expire: u32,
    pub minimum: u32,
}

impl ResourceRecord {
    /// Encode this RR into `out` with name compression. Returns bytes written.
    pub fn encode(
        &self,
        out: &mut [u8],
        absolute_offset: usize,
        ctx: &mut EncodeCtx,
    ) -> Result<usize, ParseError> {
        let mut written = 0;

        // NAME (compressed).
        written += ctx.write_name(&self.name, &mut out[written..], absolute_offset + written)?;

        // Fixed header: TYPE + CLASS + TTL + RDLENGTH (10 bytes).
        let header_start = written;
        if written + 10 > out.len() {
            return Err(ParseError::OutputBufferTooSmall {
                need: written + 10,
                have: out.len(),
            });
        }
        out[written..written + 2].copy_from_slice(&self.rtype().0.to_be_bytes());
        out[written + 2..written + 4].copy_from_slice(&self.class.0.to_be_bytes());
        out[written + 4..written + 8].copy_from_slice(&self.ttl.to_be_bytes());
        // RDLENGTH set after RDATA is written.
        written += 10;

        // RDATA.
        let rdata_start = written;
        written += self
            .data
            .encode(&mut out[written..], absolute_offset + written, ctx)?;

        let rdlength =
            u16::try_from(written - rdata_start).map_err(|_| ParseError::OutputBufferTooSmall {
                need: 0xFFFF,
                have: 0,
            })?;
        out[header_start + 8..header_start + 10].copy_from_slice(&rdlength.to_be_bytes());

        Ok(written)
    }

    /// Decode a single RR starting at `offset` in `buf`. Returns the RR + next offset.
    pub fn decode(buf: &[u8], offset: usize) -> Result<(Self, usize), ParseError> {
        let (name, mut pos) = Name::decode(buf, offset)?;
        if pos + 10 > buf.len() {
            return Err(ParseError::UnexpectedEof {
                offset: pos,
                context: "RR header",
            });
        }
        let rtype = QType(u16::from_be_bytes([buf[pos], buf[pos + 1]]));
        let class = QClass(u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]));
        let ttl = u32::from_be_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]);
        let rdlength = usize::from(u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]));
        pos += 10;
        if pos + rdlength > buf.len() {
            return Err(ParseError::UnexpectedEof {
                offset: pos,
                context: "RR rdata",
            });
        }
        let rdata_end = pos + rdlength;
        let data = RData::decode(rtype, buf, pos, rdata_end)?;
        Ok((
            Self {
                name,
                class,
                ttl,
                data,
            },
            rdata_end,
        ))
    }
}

impl RData {
    /// Encode RDATA (length prefix is the RR encoder's responsibility, not this one).
    pub fn encode(
        &self,
        out: &mut [u8],
        absolute_offset: usize,
        ctx: &mut EncodeCtx,
    ) -> Result<usize, ParseError> {
        match self {
            Self::A(addr) => write_slice(out, &addr.octets()),
            Self::Aaaa(addr) => write_slice(out, &addr.octets()),
            Self::Ns(name) | Self::Cname(name) | Self::Ptr(name) => {
                ctx.write_name(name, out, absolute_offset)
            }
            Self::Mx {
                preference,
                exchange,
            } => {
                let mut n = write_slice(out, &preference.to_be_bytes())?;
                n += ctx.write_name(exchange, &mut out[n..], absolute_offset + n)?;
                Ok(n)
            }
            Self::Txt(strings) => encode_txt(out, strings),
            Self::Soa(soa) => {
                let mut n = 0;
                n += ctx.write_name(&soa.mname, &mut out[n..], absolute_offset + n)?;
                n += ctx.write_name(&soa.rname, &mut out[n..], absolute_offset + n)?;
                for v in [soa.serial, soa.refresh, soa.retry, soa.expire, soa.minimum] {
                    n += write_slice(&mut out[n..], &v.to_be_bytes())?;
                }
                Ok(n)
            }
            Self::Srv {
                priority,
                weight,
                port,
                target,
            } => {
                let mut n = 0;
                n += write_slice(&mut out[n..], &priority.to_be_bytes())?;
                n += write_slice(&mut out[n..], &weight.to_be_bytes())?;
                n += write_slice(&mut out[n..], &port.to_be_bytes())?;
                // SRV targets are NOT compressed (RFC 2782 §"Data transmission order").
                // But per RFC 3597 receivers should tolerate pointers; we still emit
                // uncompressed for interop.
                n += target.encode(&mut out[n..])?;
                Ok(n)
            }
            Self::Caa { flags, tag, value } => {
                let mut n = 0;
                n += write_slice(&mut out[n..], &[*flags])?;
                let tag_len = u8::try_from(tag.len())
                    .map_err(|_| ParseError::LabelTooLong { len: tag.len() })?;
                n += write_slice(&mut out[n..], &[tag_len])?;
                n += write_slice(&mut out[n..], tag)?;
                n += write_slice(&mut out[n..], value)?;
                Ok(n)
            }
            Self::Unknown { data, .. } => write_slice(out, data),
        }
    }

    /// Decode RDATA of a known or unknown type.
    pub fn decode(rtype: QType, buf: &[u8], start: usize, end: usize) -> Result<Self, ParseError> {
        let slice = &buf[start..end];
        match rtype {
            QType::A => {
                if slice.len() != 4 {
                    return Err(ParseError::UnexpectedEof {
                        offset: start,
                        context: "A rdata length",
                    });
                }
                Ok(Self::A(Ipv4Addr::new(
                    slice[0], slice[1], slice[2], slice[3],
                )))
            }
            QType::AAAA => {
                if slice.len() != 16 {
                    return Err(ParseError::UnexpectedEof {
                        offset: start,
                        context: "AAAA rdata length",
                    });
                }
                let mut octets = [0u8; 16];
                octets.copy_from_slice(slice);
                Ok(Self::Aaaa(Ipv6Addr::from(octets)))
            }
            QType::NS => {
                let (n, _) = Name::decode(buf, start)?;
                Ok(Self::Ns(n))
            }
            QType::CNAME => {
                let (n, _) = Name::decode(buf, start)?;
                Ok(Self::Cname(n))
            }
            QType::PTR => {
                let (n, _) = Name::decode(buf, start)?;
                Ok(Self::Ptr(n))
            }
            QType::MX => {
                if slice.len() < 3 {
                    return Err(ParseError::UnexpectedEof {
                        offset: start,
                        context: "MX",
                    });
                }
                let preference = u16::from_be_bytes([slice[0], slice[1]]);
                let (exchange, _) = Name::decode(buf, start + 2)?;
                Ok(Self::Mx {
                    preference,
                    exchange,
                })
            }
            QType::TXT => Ok(Self::Txt(decode_txt(slice)?)),
            QType::SOA => Ok(Self::Soa(decode_soa(buf, start, end)?)),
            QType::SRV => {
                if slice.len() < 7 {
                    return Err(ParseError::UnexpectedEof {
                        offset: start,
                        context: "SRV",
                    });
                }
                let priority = u16::from_be_bytes([slice[0], slice[1]]);
                let weight = u16::from_be_bytes([slice[2], slice[3]]);
                let port = u16::from_be_bytes([slice[4], slice[5]]);
                let (target, _) = Name::decode(buf, start + 6)?;
                Ok(Self::Srv {
                    priority,
                    weight,
                    port,
                    target,
                })
            }
            QType::CAA => {
                if slice.len() < 2 {
                    return Err(ParseError::UnexpectedEof {
                        offset: start,
                        context: "CAA",
                    });
                }
                let flags = slice[0];
                let tag_len = usize::from(slice[1]);
                if slice.len() < 2 + tag_len {
                    return Err(ParseError::UnexpectedEof {
                        offset: start,
                        context: "CAA tag",
                    });
                }
                let tag = slice[2..2 + tag_len].to_vec();
                let value = slice[2 + tag_len..].to_vec();
                Ok(Self::Caa { flags, tag, value })
            }
            other => Ok(Self::Unknown {
                rtype: other,
                data: slice.to_vec(),
            }),
        }
    }
}

fn write_slice(out: &mut [u8], bytes: &[u8]) -> Result<usize, ParseError> {
    if bytes.len() > out.len() {
        return Err(ParseError::OutputBufferTooSmall {
            need: bytes.len(),
            have: out.len(),
        });
    }
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(bytes.len())
}

fn encode_txt(out: &mut [u8], strings: &[Vec<u8>]) -> Result<usize, ParseError> {
    let mut n = 0;
    for s in strings {
        let len = u8::try_from(s.len()).map_err(|_| ParseError::LabelTooLong { len: s.len() })?;
        if n + 1 + s.len() > out.len() {
            return Err(ParseError::OutputBufferTooSmall {
                need: n + 1 + s.len(),
                have: out.len(),
            });
        }
        out[n] = len;
        out[n + 1..n + 1 + s.len()].copy_from_slice(s);
        n += 1 + s.len();
    }
    Ok(n)
}

fn decode_txt(slice: &[u8]) -> Result<Vec<Vec<u8>>, ParseError> {
    let mut strings = Vec::new();
    let mut i = 0;
    while i < slice.len() {
        let len = usize::from(slice[i]);
        if i + 1 + len > slice.len() {
            return Err(ParseError::UnexpectedEof {
                offset: i,
                context: "TXT string",
            });
        }
        strings.push(slice[i + 1..i + 1 + len].to_vec());
        i += 1 + len;
    }
    Ok(strings)
}

fn decode_soa(buf: &[u8], start: usize, end: usize) -> Result<Soa, ParseError> {
    let (mname, p1) = Name::decode(buf, start)?;
    let (rname, p2) = Name::decode(buf, p1)?;
    if p2 + 20 > end {
        return Err(ParseError::UnexpectedEof {
            offset: p2,
            context: "SOA fixed fields",
        });
    }
    let get = |off: usize| u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
    Ok(Soa {
        mname,
        rname,
        serial: get(p2),
        refresh: get(p2 + 4),
        retry: get(p2 + 8),
        expire: get(p2 + 12),
        minimum: get(p2 + 16),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(rr: ResourceRecord) {
        let mut buf = vec![0u8; 512];
        let mut ctx = EncodeCtx::new();
        let n = rr.encode(&mut buf, 0, &mut ctx).unwrap();
        let (decoded, end) = ResourceRecord::decode(&buf[..n], 0).unwrap();
        assert_eq!(decoded, rr);
        assert_eq!(end, n);
    }

    #[test]
    fn roundtrip_a() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("www.example.com").unwrap(),
            class: QClass::IN,
            ttl: 300,
            data: RData::A(Ipv4Addr::new(192, 0, 2, 1)),
        });
    }

    #[test]
    fn roundtrip_aaaa() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("www.example.com").unwrap(),
            class: QClass::IN,
            ttl: 300,
            data: RData::Aaaa("2001:db8::1".parse().unwrap()),
        });
    }

    #[test]
    fn roundtrip_mx() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("example.com").unwrap(),
            class: QClass::IN,
            ttl: 3600,
            data: RData::Mx {
                preference: 10,
                exchange: Name::from_ascii("mail.example.com").unwrap(),
            },
        });
    }

    #[test]
    fn roundtrip_txt_multi_string() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("example.com").unwrap(),
            class: QClass::IN,
            ttl: 300,
            data: RData::Txt(vec![b"v=spf1 -all".to_vec(), b"key=value".to_vec()]),
        });
    }

    #[test]
    fn roundtrip_soa() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("example.com").unwrap(),
            class: QClass::IN,
            ttl: 3600,
            data: RData::Soa(Soa {
                mname: Name::from_ascii("ns1.example.com").unwrap(),
                rname: Name::from_ascii("hostmaster.example.com").unwrap(),
                serial: 2026_04_17_01,
                refresh: 10_800,
                retry: 3_600,
                expire: 604_800,
                minimum: 300,
            }),
        });
    }

    #[test]
    fn roundtrip_srv() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("_sip._tcp.example.com").unwrap(),
            class: QClass::IN,
            ttl: 60,
            data: RData::Srv {
                priority: 10,
                weight: 20,
                port: 5060,
                target: Name::from_ascii("sip.example.com").unwrap(),
            },
        });
    }

    #[test]
    fn roundtrip_caa() {
        roundtrip(ResourceRecord {
            name: Name::from_ascii("example.com").unwrap(),
            class: QClass::IN,
            ttl: 300,
            data: RData::Caa {
                flags: 0,
                tag: b"issue".to_vec(),
                value: b"letsencrypt.org".to_vec(),
            },
        });
    }

    #[test]
    fn unknown_rtype_preserved_verbatim() {
        let rr = ResourceRecord {
            name: Name::from_ascii("example.com").unwrap(),
            class: QClass::IN,
            ttl: 60,
            data: RData::Unknown {
                rtype: QType(99),
                data: vec![1, 2, 3, 4, 5],
            },
        };
        roundtrip(rr);
    }
}
