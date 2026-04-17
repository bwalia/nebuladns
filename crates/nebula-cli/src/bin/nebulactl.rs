//! `nebulactl` — NebulaDNS admin CLI entry point.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use nebula_cli::cmd::{run, Cli};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(run(cli))
}
