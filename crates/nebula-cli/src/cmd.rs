//! CLI command definitions.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Top-level CLI.
#[derive(Debug, Parser)]
#[command(name = "nebulactl", version, about = "NebulaDNS admin CLI")]
pub struct Cli {
    /// Server admin endpoint (default: `127.0.0.1:8080`).
    #[arg(long, env = "NEBULA_API", default_value = "127.0.0.1:8080")]
    pub api: SocketAddr,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Print CLI + server version information.
    Version,
    /// Probe `/livez` and `/readyz` on the admin endpoint.
    Health,
}

/// Entry point for the CLI.
pub async fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Version => version(cli.api).await,
        Cmd::Health => health(cli.api).await,
    }
}

async fn version(api: SocketAddr) -> Result<()> {
    println!("nebulactl {}", env!("CARGO_PKG_VERSION"));
    match get(api, "/api/v1/version").await {
        Ok(body) => {
            println!("server: {body}");
        }
        Err(err) => {
            eprintln!("server: unreachable ({err:#})");
        }
    }
    Ok(())
}

async fn health(api: SocketAddr) -> Result<()> {
    let livez = probe_status(api, "/livez").await?;
    let readyz = probe_status(api, "/readyz").await?;
    println!("livez:  {livez}");
    println!("readyz: {readyz}");
    if !(200..300).contains(&livez) || !(200..300).contains(&readyz) {
        anyhow::bail!("server not healthy");
    }
    Ok(())
}

async fn probe_status(addr: SocketAddr, path: &str) -> Result<u16> {
    let mut stream = TcpStream::connect(addr)
        .await
        .with_context(|| format!("connect to {addr}"))?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::with_capacity(1024);
    stream.read_to_end(&mut buf).await?;
    parse_status(&buf).with_context(|| format!("parsing response from {path}"))
}

async fn get(addr: SocketAddr, path: &str) -> Result<String> {
    let mut stream = TcpStream::connect(addr)
        .await
        .with_context(|| format!("connect to {addr}"))?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::with_capacity(4096);
    stream.read_to_end(&mut buf).await?;
    let text = std::str::from_utf8(&buf).context("non-utf8 response")?;
    let (_, body) = text.split_once("\r\n\r\n").unwrap_or(("", text));
    Ok(body.trim().to_string())
}

fn parse_status(buf: &[u8]) -> Result<u16> {
    let line = buf.split(|b| *b == b'\n').next().unwrap_or(&[]);
    let text = std::str::from_utf8(line).context("non-utf8 status line")?;
    // Expect "HTTP/1.x NNN ..."
    let code = text
        .split_whitespace()
        .nth(1)
        .context("malformed status line")?;
    code.parse::<u16>().context("status code is not u16")
}
