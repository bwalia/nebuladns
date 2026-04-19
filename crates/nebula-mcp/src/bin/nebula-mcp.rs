//! `nebula-mcp` — NebulaDNS Model Context Protocol server entry point.
//!
//! Reads JSON-RPC messages from stdin and writes responses to stdout, so it drops into
//! any MCP-aware client (Claude Desktop, Claude Code) as a stdio server.

#![forbid(unsafe_code)]

use anyhow::Result;
use nebula_mcp::{run_stdio, Config};

fn main() -> Result<()> {
    // Logs must go to stderr — stdout is the MCP protocol channel.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("NEBULA_MCP_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env()?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_stdio(cfg))
}
