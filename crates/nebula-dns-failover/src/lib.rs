//! Health-aware DNS failover controller for NebulaDNS.
//!
//! Probes configured edge POPs, applies an active-passive (or active-active)
//! policy with hysteresis and fail-open, then rewrites **managed** DNS records
//! via a pluggable provider (`nebuladns` record API first; Cloudflare optional).
//!
//! This process only changes DNS answers. It does not terminate HTTP or run
//! inside the authoritative query path. Clients discover the new target after
//! TTL + recursive-cache expiry (roughly `2–3 × TTL`).

#![forbid(unsafe_code)]

pub mod api;
pub mod apply;
pub mod audit;
pub mod config;
pub mod controller;
pub mod health;
pub mod metrics;
pub mod policy;
pub mod provider;

pub use config::Config;
pub use controller::{run_daemon, run_once, ExitCode};
