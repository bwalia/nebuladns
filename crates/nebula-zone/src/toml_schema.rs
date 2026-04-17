//! TOML schema for zone files.
//!
//! Example:
//!
//! ```toml
//! origin = "example.com."
//! default_ttl = 300
//!
//! [soa]
//! mname = "ns1.example.com."
//! rname = "hostmaster.example.com."
//! serial = 2026041701
//! refresh = 10800
//! retry = 3600
//! expire = 604800
//! minimum = 300
//!
//! [[records]]
//! name = "@"
//! type = "NS"
//! value = "ns1.example.com."
//! ```

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneDoc {
    /// Fully-qualified origin of the zone (trailing dot recommended).
    pub origin: String,
    /// Fallback TTL applied to any record that doesn't specify one.
    pub default_ttl: u32,
    pub soa: Soa,
    #[serde(default)]
    pub records: Vec<Record>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Soa {
    pub mname: String,
    pub rname: String,
    pub serial: u32,
    pub refresh: u32,
    pub retry: u32,
    pub expire: u32,
    pub minimum: u32,
    pub ttl: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Owner name: `"@"` for apex, bare `"foo"` for relative, trailing-dot for absolute.
    pub name: String,
    /// Record type (`"A"`, `"AAAA"`, `"NS"`, `"CNAME"`, `"MX"`, `"TXT"`, `"PTR"`).
    #[serde(rename = "type")]
    pub rtype: String,
    /// Record value in canonical presentation format.
    pub value: String,
    /// Per-record TTL override. Falls back to the zone's `default_ttl`.
    pub ttl: Option<u32>,
}
