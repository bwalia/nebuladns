//! `nebuladns` — the server binary.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use nebula_server::run::{main as run_main, Args};

fn main() -> Result<()> {
    let args = Args::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("nebula-worker")
        .build()?;
    rt.block_on(run_main(args))
}
