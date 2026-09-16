//! Presentation-format RDATA parse/print for the TOML loader and the record API.

use nebula_wire::RData;

use crate::{parse_name, ZoneError};

/// Parse a single RDATA value from its mnemonic type and presentation string.
pub fn parse_rdata(rtype: &str, value: &str) -> Result<RData, ZoneError> {
    let rtype_upper = rtype.to_ascii_uppercase();
    let rdata_err = |msg: String| ZoneError::Rdata {
        owner: String::new(),
        rtype: rtype_upper.clone(),
        msg,
    };
    match rtype_upper.as_str() {
        "A" => Ok(RData::A(value.parse().map_err(
            |e: std::net::AddrParseError| rdata_err(e.to_string()),
        )?)),
        "AAAA" => {
            Ok(RData::Aaaa(value.parse().map_err(
                |e: std::net::AddrParseError| rdata_err(e.to_string()),
            )?))
        }
        "NS" => Ok(RData::Ns(parse_name(value)?)),
        "CNAME" => Ok(RData::Cname(parse_name(value)?)),
        "PTR" => Ok(RData::Ptr(parse_name(value)?)),
        "TXT" => Ok(RData::Txt(vec![value.as_bytes().to_vec()])),
        "MX" => {
            let (pref, exch) = value
                .split_once(' ')
                .ok_or_else(|| rdata_err("expected `<preference> <exchange>`".into()))?;
            let preference: u16 = pref
                .parse()
                .map_err(|e: std::num::ParseIntError| rdata_err(e.to_string()))?;
            Ok(RData::Mx {
                preference,
                exchange: parse_name(exch.trim())?,
            })
        }
        "SRV" => {
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() != 4 {
                return Err(rdata_err(
                    "expected `<priority> <weight> <port> <target>`".into(),
                ));
            }
            let parse_u16 = |s: &str| {
                s.parse::<u16>()
                    .map_err(|e: std::num::ParseIntError| rdata_err(e.to_string()))
            };
            Ok(RData::Srv {
                priority: parse_u16(parts[0])?,
                weight: parse_u16(parts[1])?,
                port: parse_u16(parts[2])?,
                target: parse_name(parts[3])?,
            })
        }
        "CAA" => {
            let (flags_s, rest) = value
                .split_once(' ')
                .ok_or_else(|| rdata_err("expected `<flags> <tag> <value>`".into()))?;
            let (tag, val) = rest
                .split_once(' ')
                .ok_or_else(|| rdata_err("expected `<flags> <tag> <value>`".into()))?;
            let flags: u8 = flags_s
                .parse()
                .map_err(|e: std::num::ParseIntError| rdata_err(e.to_string()))?;
            Ok(RData::Caa {
                flags,
                tag: tag.as_bytes().to_vec(),
                value: val.as_bytes().to_vec(),
            })
        }
        _ => Err(rdata_err("unsupported record type".into())),
    }
}

/// Render RDATA in the same presentation form the TOML loader accepts.
#[must_use]
pub fn rdata_presentation(data: &RData) -> String {
    match data {
        RData::A(ip) => ip.to_string(),
        RData::Aaaa(ip) => ip.to_string(),
        RData::Ns(n) | RData::Cname(n) | RData::Ptr(n) => n.to_ascii(),
        RData::Mx {
            preference,
            exchange,
        } => format!("{preference} {}", exchange.to_ascii()),
        RData::Txt(chunks) => {
            let mut s = String::new();
            for c in chunks {
                s.push_str(&String::from_utf8_lossy(c));
            }
            s
        }
        RData::Soa(soa) => format!(
            "{} {} {} {} {} {} {}",
            soa.mname.to_ascii(),
            soa.rname.to_ascii(),
            soa.serial,
            soa.refresh,
            soa.retry,
            soa.expire,
            soa.minimum
        ),
        RData::Srv {
            priority,
            weight,
            port,
            target,
        } => format!("{priority} {weight} {port} {}", target.to_ascii()),
        RData::Caa { flags, tag, value } => format!(
            "{flags} {} {}",
            String::from_utf8_lossy(tag),
            String::from_utf8_lossy(value)
        ),
        RData::Unknown { data, .. } => format!("\\# {} {}", data.len(), hex_encode(data)),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_cname() {
        let d = parse_rdata("cname", "lb.example.net.").unwrap();
        assert!(matches!(d, RData::Cname(_)));
        assert_eq!(rdata_presentation(&d), "lb.example.net.");
    }

    #[test]
    fn a_parse_rejects_garbage() {
        assert!(parse_rdata("A", "not-an-ip").is_err());
    }
}
