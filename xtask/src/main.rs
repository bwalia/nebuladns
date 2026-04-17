//! Dev-loop tasks: `cargo xtask <subcommand>`.
//!
//! Run `cargo xtask --help` for the catalogue.

#![forbid(unsafe_code)]

use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "xtask", about = "NebulaDNS dev-loop tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run the full local CI suite (fmt, clippy, test, deny).
    Ci,
    /// Run `cargo fmt --all --check`.
    Fmt,
    /// Run `cargo clippy --workspace --all-targets -- -D warnings`.
    Clippy,
    /// Run `cargo nextest run --workspace` (falls back to `cargo test`).
    Test,
    /// Run `cargo deny check`.
    Deny,
    /// Smoke-run a fuzz target for `$duration` seconds (default 60).
    Fuzz {
        /// Target name, e.g. `wire_roundtrip`.
        target: String,
        /// Crate that owns the fuzz target (default `nebula-wire`).
        #[arg(long, default_value = "nebula-wire")]
        krate: String,
        /// Duration in seconds.
        #[arg(long, default_value_t = 60)]
        duration: u64,
    },
    /// Run all benchmarks.
    Bench,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Ci => {
            run("cargo", &["fmt", "--all", "--check"])?;
            run(
                "cargo",
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            try_run("cargo", &["nextest", "run", "--workspace"])
                .or_else(|_| run("cargo", &["test", "--workspace"]))?;
            // `cargo deny` may not be installed locally — warn, don't fail hard.
            let _ = try_run("cargo", &["deny", "check"]);
        }
        Cmd::Fmt => run("cargo", &["fmt", "--all", "--check"])?,
        Cmd::Clippy => run(
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        )?,
        Cmd::Test => {
            try_run("cargo", &["nextest", "run", "--workspace"])
                .or_else(|_| run("cargo", &["test", "--workspace"]))?;
        }
        Cmd::Deny => run("cargo", &["deny", "check"])?,
        Cmd::Fuzz {
            target,
            krate,
            duration,
        } => {
            let secs = duration.to_string();
            let manifest = format!("crates/{krate}/fuzz/Cargo.toml");
            run(
                "cargo",
                &[
                    "fuzz",
                    "run",
                    &target,
                    "--fuzz-dir",
                    &format!("crates/{krate}/fuzz"),
                    "--",
                    "-max_total_time",
                    &secs,
                    "-print_final_stats=1",
                ],
            )
            .or_else(|_| {
                run(
                    "cargo",
                    &[
                        "fuzz",
                        "run",
                        &target,
                        "--manifest-path",
                        &manifest,
                        "--",
                        "-max_total_time",
                        &secs,
                    ],
                )
            })?;
        }
        Cmd::Bench => run("cargo", &["bench", "--workspace"])?,
    }
    Ok(())
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to spawn `{program} {}`", args.join(" ")))?;
    if !status.success() {
        bail!("`{program} {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

fn try_run(program: &str, args: &[&str]) -> Result<()> {
    run(program, args)
}
