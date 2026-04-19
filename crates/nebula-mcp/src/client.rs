//! Minimal HTTP/1.0 client for talking to `nebula-api`.
//!
//! We mirror `nebulactl`'s raw-TCP approach (see `crates/nebula-cli/src/cmd.rs`) rather
//! than pulling in `reqwest` or `hyper-util` for two reasons:
//!
//! 1. **No new workspace deps.** `reqwest` is not listed in `workspace.dependencies`, so
//!    adding it widens the `cargo deny` surface for what ends up being a dozen lines of
//!    request/response glue. When the project needs a real HTTP client it should add one
//!    in one place and consumers (CLI + MCP) should both migrate together.
//! 2. **Consistency.** The CLI and the MCP server are two client shapes over the same API
//!    — keeping the transport identical means bugs surface in one place.
//!
//! The API is loopback-only today and the server speaks HTTP/1.0 without keepalive, so
//! the tradeoffs we'd normally get from a real client (connection pooling, HTTP/2, TLS)
//! aren't in play.

use std::fmt::Write as _;
use std::net::SocketAddr;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Connection info for a `nebula-api` instance.
#[derive(Debug, Clone)]
pub struct ApiClient {
    pub addr: SocketAddr,
    /// Optional bearer token. M5+ will require auth; passing `None` today is fine.
    pub token: Option<String>,
}

impl ApiClient {
    pub fn new(addr: SocketAddr, token: Option<String>) -> Self {
        Self { addr, token }
    }

    pub async fn get_text(&self, path: &str) -> Result<HttpResponse> {
        self.send("GET", path, None).await
    }

    pub async fn get_json(&self, path: &str) -> Result<Value> {
        let resp = self.get_text(path).await?;
        resp.expect_success()?;
        serde_json::from_str(&resp.body).with_context(|| format!("parse JSON response from {path}"))
    }

    pub async fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let serialized = serde_json::to_string(body)?;
        let resp = self
            .send("POST", path, Some(("application/json", serialized)))
            .await?;
        resp.expect_success()?;
        if resp.body.trim().is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&resp.body)
                .with_context(|| format!("parse JSON response from {path}"))
        }
    }

    pub async fn put_json(&self, path: &str, body: &Value) -> Result<Value> {
        let serialized = serde_json::to_string(body)?;
        let resp = self
            .send("PUT", path, Some(("application/json", serialized)))
            .await?;
        resp.expect_success()?;
        if resp.body.trim().is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&resp.body)
                .with_context(|| format!("parse JSON response from {path}"))
        }
    }

    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<(&str, String)>,
    ) -> Result<HttpResponse> {
        let mut stream = TcpStream::connect(self.addr)
            .await
            .with_context(|| format!("connect to {}", self.addr))?;

        let mut req = format!(
            "{method} {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\nAccept: application/json\r\n",
            host = self.addr
        );
        if let Some(token) = &self.token {
            write!(req, "Authorization: Bearer {token}\r\n")?;
        }
        if let Some((content_type, ref body)) = body {
            write!(req, "Content-Type: {content_type}\r\n")?;
            write!(req, "Content-Length: {}\r\n", body.len())?;
        }
        req.push_str("\r\n");
        if let Some((_, body)) = body {
            req.push_str(&body);
        }

        stream.write_all(req.as_bytes()).await?;
        let mut buf = Vec::with_capacity(8192);
        stream.read_to_end(&mut buf).await?;

        parse_response(&buf)
    }
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    /// Treat 2xx as success; everything else surfaces as an error with the body included
    /// so the MCP tool result carries a useful message back to the model.
    pub fn expect_success(&self) -> Result<()> {
        if (200..300).contains(&self.status) {
            Ok(())
        } else {
            bail!("HTTP {}: {}", self.status, self.body.trim())
        }
    }
}

fn parse_response(buf: &[u8]) -> Result<HttpResponse> {
    let text = std::str::from_utf8(buf).context("non-utf8 response")?;
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or(("", text));
    let status_line = head.lines().next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .context("malformed status line")?
        .parse::<u16>()
        .context("status code is not u16")?;
    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok_body() {
        let raw = b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{\"a\":1}";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, "{\"a\":1}");
        r.expect_success().unwrap();
    }

    #[test]
    fn parse_error_body() {
        let raw = b"HTTP/1.0 404 Not Found\r\n\r\nnope";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 404);
        assert!(r.expect_success().is_err());
    }

    #[test]
    fn parse_empty_body() {
        let raw = b"HTTP/1.0 204 No Content\r\n\r\n";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 204);
        assert!(r.body.is_empty());
    }
}
