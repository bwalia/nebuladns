//! In-place RRset mutation with RFC-facing safety checks.
//!
//! These rules exist so a fast record API cannot silently corrupt a zone:
//!
//! - SOA is never mutated through this path (serial bump is internal).
//! - Apex NS is never mutated through this path (delegation identity).
//! - CNAME at the apex is rejected (SOA + NS already occupy the name).
//! - CNAME cannot coexist with any other type at the same owner (RFC 1034 §3.6.2).
//! - CNAME RRsets are singletons.
//! - Owners must fall inside the zone origin.

use nebula_wire::{Name, QClass, QType, RData, ResourceRecord};

use crate::{IndexKey, Zone, ZoneError};

impl Zone {
    /// Current SOA serial.
    #[must_use]
    pub fn serial(&self) -> u32 {
        match &self.soa.data {
            RData::Soa(soa) => soa.serial,
            _ => 0,
        }
    }

    /// Replace the RRset at `(owner, type)` with `data`. All `RData` values must share
    /// one type. Bumps the SOA serial on success.
    pub fn upsert_rrset(
        &mut self,
        owner: Name,
        ttl: u32,
        data: Vec<RData>,
    ) -> Result<UpsertOutcome, ZoneError> {
        if data.is_empty() {
            return Err(ZoneError::EmptyRRset);
        }
        let rtype = data[0].rtype();
        if data.iter().any(|d| d.rtype() != rtype) {
            return Err(ZoneError::MixedRRset);
        }
        self.validate_mutation(
            &owner,
            rtype,
            /*is_cname_insert=*/ rtype == QType::CNAME,
        )?;
        if rtype == QType::CNAME && data.len() != 1 {
            return Err(ZoneError::CnameNotSingleton);
        }

        let owner_lower = owner.to_ascii_lowercase();
        let previous = self
            .index
            .get(&IndexKey {
                owner: owner_lower.clone(),
                qtype: rtype,
            })
            .map(Vec::as_slice)
            .map(|rrs| rrs.iter().map(|rr| rr.data.clone()).collect::<Vec<_>>());

        let records: Vec<ResourceRecord> = data
            .into_iter()
            .map(|d| ResourceRecord {
                name: owner.clone(),
                class: QClass::IN,
                ttl,
                data: d,
            })
            .collect();
        self.index.insert(
            IndexKey {
                owner: owner_lower,
                qtype: rtype,
            },
            records,
        );
        let serial = self.bump_serial();
        Ok(UpsertOutcome {
            rtype,
            previous,
            serial,
        })
    }

    /// Remove the RRset at `(owner, type)`. Bumps the SOA serial on success.
    pub fn delete_rrset(&mut self, owner: &Name, qtype: QType) -> Result<DeleteOutcome, ZoneError> {
        self.validate_mutation(owner, qtype, false)?;
        let key = IndexKey {
            owner: owner.to_ascii_lowercase(),
            qtype,
        };
        let removed = self
            .index
            .remove(&key)
            .ok_or_else(|| ZoneError::NoSuchRRset {
                owner: owner.to_ascii(),
                rtype: qtype.mnemonic().unwrap_or("TYPE?").to_string(),
            })?;
        let serial = self.bump_serial();
        Ok(DeleteOutcome { removed, serial })
    }

    fn validate_mutation(
        &self,
        owner: &Name,
        rtype: QType,
        is_cname_insert: bool,
    ) -> Result<(), ZoneError> {
        let origin_lower = self.origin.to_ascii_lowercase();
        let owner_lower = owner.to_ascii_lowercase();
        if !owner_lower.ends_with_name(&origin_lower) {
            return Err(ZoneError::OwnerOutsideOrigin {
                owner: owner.to_ascii(),
                origin: self.origin.to_ascii(),
            });
        }
        if rtype == QType::SOA {
            return Err(ZoneError::ProtectedRRset {
                owner: owner.to_ascii(),
                rtype: "SOA".into(),
            });
        }
        if rtype == QType::NS && owner_lower == origin_lower {
            return Err(ZoneError::ProtectedRRset {
                owner: owner.to_ascii(),
                rtype: "NS".into(),
            });
        }
        if is_cname_insert && owner_lower == origin_lower {
            return Err(ZoneError::CnameAtApex {
                origin: self.origin.to_ascii(),
            });
        }

        // CNAME exclusivity at this owner.
        let other_types: Vec<QType> = self
            .index
            .keys()
            .filter(|k| k.owner == owner_lower && k.qtype != rtype)
            .map(|k| k.qtype)
            .collect();

        if is_cname_insert && !other_types.is_empty() {
            return Err(ZoneError::CnameConflict {
                owner: owner.to_ascii(),
            });
        }
        if !is_cname_insert && other_types.contains(&QType::CNAME) {
            return Err(ZoneError::CnameConflict {
                owner: owner.to_ascii(),
            });
        }
        Ok(())
    }

    fn bump_serial(&mut self) -> u32 {
        let mut soa_rr = self.soa.clone();
        let serial = if let RData::Soa(soa) = &mut soa_rr.data {
            soa.serial = soa.serial.wrapping_add(1);
            if soa.serial == 0 {
                soa.serial = 1;
            }
            soa.serial
        } else {
            1
        };
        let key = IndexKey {
            owner: self.origin.to_ascii_lowercase(),
            qtype: QType::SOA,
        };
        self.index.insert(key, vec![soa_rr.clone()]);
        self.soa = soa_rr;
        serial
    }
}

/// Result of a successful upsert.
#[derive(Debug, Clone)]
pub struct UpsertOutcome {
    pub rtype: QType,
    /// Previous RDATA at this owner/type, if any.
    pub previous: Option<Vec<RData>>,
    pub serial: u32,
}

/// Result of a successful delete.
#[derive(Debug, Clone)]
pub struct DeleteOutcome {
    pub removed: Vec<ResourceRecord>,
    pub serial: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdata::parse_rdata;
    use crate::Zone;

    const SAMPLE: &str = r#"
origin = "example.com."
default_ttl = 300
[soa]
mname = "ns1.example.com."
rname = "hostmaster.example.com."
serial = 1
refresh = 10800
retry = 3600
expire = 604800
minimum = 300
[[records]]
name = "@"
type = "NS"
value = "ns1.example.com."
[[records]]
name = "www"
type = "A"
value = "192.0.2.10"
"#;

    fn zone() -> Zone {
        Zone::from_toml(SAMPLE).unwrap()
    }

    #[test]
    fn upsert_cname_and_bump_serial() {
        let mut z = zone();
        let owner = Name::from_ascii("app.example.com").unwrap();
        let out = z
            .upsert_rrset(
                owner.clone(),
                5,
                vec![parse_rdata("CNAME", "west.example.net.").unwrap()],
            )
            .unwrap();
        assert_eq!(out.serial, 2);
        let rrset = z.find(&owner, QType::CNAME).unwrap();
        assert_eq!(rrset[0].ttl, 5);
        assert_eq!(z.serial(), 2);
    }

    #[test]
    fn retarget_cname_replaces_rrset() {
        let mut z = zone();
        let owner = Name::from_ascii("app.example.com").unwrap();
        z.upsert_rrset(
            owner.clone(),
            5,
            vec![parse_rdata("CNAME", "west.example.net.").unwrap()],
        )
        .unwrap();
        z.upsert_rrset(
            owner.clone(),
            5,
            vec![parse_rdata("CNAME", "east.example.net.").unwrap()],
        )
        .unwrap();
        let rrset = z.find(&owner, QType::CNAME).unwrap();
        assert_eq!(rrset.len(), 1);
        match &rrset[0].data {
            RData::Cname(n) => assert_eq!(n.to_ascii(), "east.example.net."),
            _ => panic!("expected CNAME"),
        }
    }

    #[test]
    fn cname_at_apex_rejected() {
        let mut z = zone();
        let err = z
            .upsert_rrset(
                z.origin().clone(),
                5,
                vec![parse_rdata("CNAME", "elsewhere.example.net.").unwrap()],
            )
            .unwrap_err();
        assert!(matches!(err, ZoneError::CnameAtApex { .. }));
    }

    #[test]
    fn cname_cannot_coexist_with_a() {
        let mut z = zone();
        let www = Name::from_ascii("www.example.com").unwrap();
        let err = z
            .upsert_rrset(
                www,
                5,
                vec![parse_rdata("CNAME", "other.example.net.").unwrap()],
            )
            .unwrap_err();
        assert!(matches!(err, ZoneError::CnameConflict { .. }));
    }

    #[test]
    fn soa_and_apex_ns_are_protected() {
        let mut z = zone();
        let apex = z.origin().clone();
        assert!(matches!(
            z.delete_rrset(&apex, QType::SOA).unwrap_err(),
            ZoneError::ProtectedRRset { .. }
        ));
        assert!(matches!(
            z.delete_rrset(&apex, QType::NS).unwrap_err(),
            ZoneError::ProtectedRRset { .. }
        ));
    }

    #[test]
    fn owner_outside_zone_rejected() {
        let mut z = zone();
        let err = z
            .upsert_rrset(
                Name::from_ascii("www.not-example.com").unwrap(),
                5,
                vec![parse_rdata("A", "192.0.2.99").unwrap()],
            )
            .unwrap_err();
        assert!(matches!(err, ZoneError::OwnerOutsideOrigin { .. }));
    }

    #[test]
    fn delete_then_add_cname_over_former_a() {
        let mut z = zone();
        let www = Name::from_ascii("www.example.com").unwrap();
        z.delete_rrset(&www, QType::A).unwrap();
        z.upsert_rrset(
            www.clone(),
            5,
            vec![parse_rdata("CNAME", "cdn.example.net.").unwrap()],
        )
        .unwrap();
        assert!(z.find(&www, QType::A).is_none());
        assert_eq!(z.find(&www, QType::CNAME).unwrap().len(), 1);
    }
}
