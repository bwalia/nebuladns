//! In-memory zones + TOML loader.
//!
//! M1 scope: a single zone lives in memory as a hash-indexed collection of RRsets keyed by
//! `(lowercased owner, qtype)`. The primary file format is native TOML — familiar to ops
//! people and directly round-trippable with our data model. Master-file (RFC 1035 §5),
//! YAML, and JSON loaders land in M5.

#![forbid(unsafe_code)]

pub mod mutate;
pub mod rdata;
pub mod registry;
pub mod toml_schema;

use std::collections::HashMap;

use nebula_wire::{Name, QClass, QType, RData, ResourceRecord};
use thiserror::Error;

use crate::toml_schema::ZoneDoc;

pub use mutate::{DeleteOutcome, UpsertOutcome};
pub use rdata::{parse_rdata, rdata_presentation};
pub use registry::ZoneRegistry;

/// Error raised while loading or validating a zone.
#[derive(Debug, Error)]
pub enum ZoneError {
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid name {name:?}: {source}")]
    Name {
        name: String,
        #[source]
        source: nebula_wire::ParseError,
    },
    #[error("record {owner} has invalid rdata for {rtype}: {msg}")]
    Rdata {
        owner: String,
        rtype: String,
        msg: String,
    },
    #[error("record owner {owner:?} is not inside the zone origin {origin:?}")]
    OwnerOutsideOrigin { owner: String, origin: String },
    #[error("unknown zone {zone}")]
    UnknownZone { zone: String },
    #[error("refusing to mutate protected RRset {owner} {rtype}")]
    ProtectedRRset { owner: String, rtype: String },
    #[error("CNAME is not allowed at the zone apex {origin}")]
    CnameAtApex { origin: String },
    #[error("CNAME cannot coexist with other record types at {owner}")]
    CnameConflict { owner: String },
    #[error("CNAME RRset must contain exactly one target")]
    CnameNotSingleton,
    #[error("RRset must contain at least one record")]
    EmptyRRset,
    #[error("RRset values must all share the same type")]
    MixedRRset,
    #[error("no such RRset {owner} {rtype}")]
    NoSuchRRset { owner: String, rtype: String },
}

/// A loaded, indexed zone.
///
/// Reads of answers never allocate — the index returns borrowed slices.
#[derive(Debug, Clone)]
pub struct Zone {
    origin: Name,
    // Lowercased owner + QType → all RRs sharing that owner/type/class.
    // Same `(owner, class)` with different types are separate entries.
    pub(crate) index: HashMap<IndexKey, Vec<ResourceRecord>>,
    soa: ResourceRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct IndexKey {
    pub owner: Name,
    pub qtype: QType,
}

impl Zone {
    /// The zone's origin (apex).
    #[must_use]
    pub fn origin(&self) -> &Name {
        &self.origin
    }

    /// SOA record for this zone.
    #[must_use]
    pub fn soa(&self) -> &ResourceRecord {
        &self.soa
    }

    /// Iterate every RR in the zone.
    pub fn iter(&self) -> impl Iterator<Item = &ResourceRecord> {
        self.index.values().flat_map(|v| v.iter())
    }

    /// Return the RRset matching `(owner, qtype)`. Owner is compared case-insensitively.
    #[must_use]
    pub fn find(&self, owner: &Name, qtype: QType) -> Option<&[ResourceRecord]> {
        let key = IndexKey {
            owner: owner.to_ascii_lowercase(),
            qtype,
        };
        self.index.get(&key).map(Vec::as_slice)
    }

    /// Build a zone from a parsed TOML document.
    pub fn from_doc(doc: ZoneDoc) -> Result<Self, ZoneError> {
        let origin = parse_name(&doc.origin)?;
        let origin_lower = origin.to_ascii_lowercase();

        let mut index: HashMap<IndexKey, Vec<ResourceRecord>> = HashMap::new();

        // SOA first.
        let soa_rr = ResourceRecord {
            name: origin.clone(),
            class: QClass::IN,
            ttl: doc.soa.ttl.unwrap_or(doc.default_ttl),
            data: RData::Soa(nebula_wire::Soa {
                mname: parse_name(&doc.soa.mname)?,
                rname: parse_name(&doc.soa.rname)?,
                serial: doc.soa.serial,
                refresh: doc.soa.refresh,
                retry: doc.soa.retry,
                expire: doc.soa.expire,
                minimum: doc.soa.minimum,
            }),
        };
        insert(&mut index, soa_rr.clone());
        let soa = soa_rr;

        for rec in doc.records {
            let owner_full = qualify_name(&rec.name, &origin)?;
            if !owner_full
                .to_ascii_lowercase()
                .ends_with_name(&origin_lower)
            {
                return Err(ZoneError::OwnerOutsideOrigin {
                    owner: rec.name,
                    origin: doc.origin,
                });
            }
            let ttl = rec.ttl.unwrap_or(doc.default_ttl);
            let rr = convert_record(owner_full, ttl, rec)?;
            insert(&mut index, rr);
        }

        Ok(Self { origin, index, soa })
    }

    /// Parse a TOML zone document and load it.
    pub fn from_toml(input: &str) -> Result<Self, ZoneError> {
        let doc: ZoneDoc = toml::from_str(input)?;
        Self::from_doc(doc)
    }
}

fn insert(index: &mut HashMap<IndexKey, Vec<ResourceRecord>>, rr: ResourceRecord) {
    let key = IndexKey {
        owner: rr.name.to_ascii_lowercase(),
        qtype: rr.rtype(),
    };
    index.entry(key).or_default().push(rr);
}

fn parse_name(s: &str) -> Result<Name, ZoneError> {
    Name::from_ascii(s).map_err(|source| ZoneError::Name {
        name: s.to_string(),
        source,
    })
}

/// Resolve a relative or absolute record name against the origin.
///
/// - `"@"` → the origin itself
/// - `"foo"` (no trailing dot) → `foo.<origin>`
/// - `"foo.example.com."` (trailing dot) → absolute; origin ignored
pub fn qualify_name(name: &str, origin: &Name) -> Result<Name, ZoneError> {
    if name == "@" {
        return Ok(origin.clone());
    }
    if name.ends_with('.') {
        return parse_name(name);
    }
    // Relative: append origin labels (without the trailing dot of `to_ascii`).
    let origin_ascii = origin.to_ascii();
    let full = if origin.is_root() {
        name.to_string()
    } else {
        format!("{name}.{}", origin_ascii.trim_end_matches('.'))
    };
    parse_name(&full)
}

fn convert_record(
    owner: Name,
    ttl: u32,
    rec: toml_schema::Record,
) -> Result<ResourceRecord, ZoneError> {
    let data = parse_rdata(&rec.rtype, &rec.value).map_err(|e| match e {
        ZoneError::Rdata { rtype, msg, .. } => ZoneError::Rdata {
            owner: owner.to_ascii(),
            rtype,
            msg,
        },
        other => other,
    })?;
    Ok(ResourceRecord {
        name: owner,
        class: QClass::IN,
        ttl,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
origin = "example.com."
default_ttl = 300

[soa]
mname = "ns1.example.com."
rname = "hostmaster.example.com."
serial = 2026041701
refresh = 10800
retry = 3600
expire = 604800
minimum = 300

[[records]]
name = "@"
type = "NS"
value = "ns1.example.com."

[[records]]
name = "@"
type = "NS"
value = "ns2.example.com."

[[records]]
name = "ns1"
type = "A"
value = "192.0.2.1"

[[records]]
name = "ns2"
type = "A"
value = "192.0.2.2"

[[records]]
name = "www"
type = "A"
value = "192.0.2.10"

[[records]]
name = "www"
type = "A"
value = "192.0.2.11"

[[records]]
name = "www"
type = "AAAA"
value = "2001:db8::10"

[[records]]
name = "@"
type = "MX"
value = "10 mail.example.com."

[[records]]
name = "mail"
type = "A"
value = "192.0.2.25"
"#;

    #[test]
    fn loads_sample_zone() {
        let z = Zone::from_toml(SAMPLE).unwrap();
        assert_eq!(z.origin(), &Name::from_ascii("example.com").unwrap());
        let www = Name::from_ascii("www.example.com").unwrap();
        let a = z.find(&www, QType::A).unwrap();
        assert_eq!(a.len(), 2);
        let aaaa = z.find(&www, QType::AAAA).unwrap();
        assert_eq!(aaaa.len(), 1);
    }

    #[test]
    fn case_insensitive_lookup() {
        let z = Zone::from_toml(SAMPLE).unwrap();
        let mixed = Name::from_ascii("WwW.Example.CoM").unwrap();
        let a = z.find(&mixed, QType::A).unwrap();
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn ns_rrset_present() {
        let z = Zone::from_toml(SAMPLE).unwrap();
        let apex = Name::from_ascii("example.com").unwrap();
        let ns = z.find(&apex, QType::NS).unwrap();
        assert_eq!(ns.len(), 2);
    }

    #[test]
    fn soa_present() {
        let z = Zone::from_toml(SAMPLE).unwrap();
        let apex = Name::from_ascii("example.com").unwrap();
        assert!(z.find(&apex, QType::SOA).is_some());
        match &z.soa().data {
            RData::Soa(soa) => {
                assert_eq!(soa.serial, 2026_04_17_01);
            }
            _ => panic!("expected SOA"),
        }
    }
}
