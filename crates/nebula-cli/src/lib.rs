//! `nebulactl` — NebulaDNS admin CLI.
//!
//! M0 ships two subcommands (`version`, `health`). The full `zone / deploy / secondaries
//! / keys` command tree arrives in M5 alongside the REST API.

#![forbid(unsafe_code)]

pub mod cmd;
