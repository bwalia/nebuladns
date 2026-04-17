//! Propagation verifier.
//!
//! Polls declared downstream secondaries until every required peer reports the current SOA
//! serial. A deploy is not "complete" until this verifier confirms it.

#![forbid(unsafe_code)]

pub const MARKER: &str = "nebula-verify";
