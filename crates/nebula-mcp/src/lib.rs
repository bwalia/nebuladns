//! `nebula-mcp` — Model Context Protocol server for NebulaDNS.
//!
//! Exposes NebulaDNS admin operations as MCP tools so that Claude (Desktop, Code, or any
//! other MCP client) can drive zone management through `nebula-api`. The server speaks
//! JSON-RPC 2.0 over stdio per the MCP specification.
//!
//! # Design
//!
//! - Tools call `nebula-api` over HTTP, mirroring what `nebulactl` does. This keeps the
//!   API boundary authoritative — auth, audit, and RBAC all apply to MCP callers exactly
//!   as they apply to CLI callers.
//! - Mutating tools are gated behind `NEBULA_MCP_ALLOW_WRITES=1`. Without the gate the
//!   server still advertises write tools, but invoking one returns an error explaining
//!   how to enable writes. This lets operators drop the MCP server into Claude without
//!   fear of unintended DNS mutations.
//! - No third-party MCP SDK. The protocol surface we use is small (a handful of JSON-RPC
//!   methods) and staying in-workspace keeps `cargo deny` happy.

#![forbid(unsafe_code)]

pub mod client;
pub mod protocol;
pub mod server;
pub mod tools;

pub use server::{run_stdio, Config};
