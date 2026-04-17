//! NebulaDNS server runtime.
//!
//! M0 scope: config loading, tracing setup, control-plane HTTP, metrics HTTP, graceful
//! shutdown, systemd sd_notify integration. DNS listeners land in M1–M2.

#![forbid(unsafe_code)]

pub mod config;
pub mod dns;
pub mod notify;
pub mod run;
pub mod shutdown;
pub mod telemetry;
