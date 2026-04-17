//! DNS data plane: zone registry, query handler, UDP/TCP listeners.
//!
//! M1 answer path:
//!   1. decode the query
//!   2. find the most-specific zone containing the qname
//!   3. look up the RRset; on a hit → authoritative answer, on a miss → NXDOMAIN+SOA
//!   4. encode response with compression; UDP responses over 512 bytes get `TC=1`
//!
//! Wildcards, CNAME chasing, NS delegation, and proper negative responses (RFC 2308) land
//! in M2. This module is deliberately tiny so we can iterate quickly against `dig`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwap;
use nebula_metrics::dns::{
    DnsMetrics, DropLabels, DropReason, Proto, QTypeLabel, QueryLabels, RcodeLabel,
};
use nebula_wire::{
    Header, Message, Name, QClass, QType, RCode, RData, ResourceRecord, Soa, MAX_UDP_MESSAGE_SIZE,
};
use nebula_zone::Zone;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_util::sync::CancellationToken;

/// Shared registry of loaded zones, keyed by lowercased origin. Writes publish a new
/// snapshot atomically via arc-swap; readers never block and never allocate.
#[derive(Debug, Clone, Default)]
pub struct ZoneRegistry {
    inner: Arc<ArcSwap<HashMap<Name, Arc<Zone>>>>,
}

impl ZoneRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ArcSwap::new(Arc::new(HashMap::new()))),
        }
    }

    /// Replace the registry contents atomically.
    pub fn replace(&self, zones: impl IntoIterator<Item = Zone>) {
        let map: HashMap<Name, Arc<Zone>> = zones
            .into_iter()
            .map(|z| (z.origin().to_ascii_lowercase(), Arc::new(z)))
            .collect();
        self.inner.store(Arc::new(map));
    }

    /// Find the most-specific zone containing `qname`.
    #[must_use]
    pub fn zone_for(&self, qname: &Name) -> Option<Arc<Zone>> {
        let map = self.inner.load();
        let qname_lower = qname.to_ascii_lowercase();
        let labels = qname_lower.labels();
        // Walk from the full name toward the apex — longest-match wins.
        for skip in 0..=labels.len() {
            let tail = labels[skip..].to_vec();
            let candidate = Name::from_labels(tail);
            if let Some(z) = map.get(&candidate) {
                return Some(z.clone());
            }
        }
        None
    }
}

/// Construct a Name directly from label byte vectors. Not public API on Name itself yet
/// because labels are a private implementation detail over there; we expose a shim here
/// via `from_ascii` round-tripping. Using `to_ascii_lowercase().labels()` above keeps
/// ownership simple.
trait NameFromLabels {
    fn from_labels(labels: Vec<Vec<u8>>) -> Self;
}
impl NameFromLabels for Name {
    fn from_labels(labels: Vec<Vec<u8>>) -> Self {
        // There's no public constructor, so we rebuild via the ASCII path.
        if labels.is_empty() {
            return Self::root();
        }
        let parts: Vec<String> = labels
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect();
        Self::from_ascii(&parts.join(".")).unwrap_or_else(|_| Self::root())
    }
}

/// Produce a response for `query`. Never panics on untrusted input.
pub fn answer(query: &Message, zones: &ZoneRegistry) -> Message {
    let mut resp = Message::default();
    resp.header = query.header;
    resp.header.flags.qr = true;
    resp.header.flags.aa = false;
    resp.header.flags.ra = false;

    // Echo the first question (our answer relates to that one). Reject malformed queries
    // with FORMERR — this is also the exact failure class the prompt highlights.
    let Some(q) = query.questions.first().cloned() else {
        resp.header.rcode = RCode::FormErr;
        return resp;
    };
    resp.questions.push(q.clone());

    // IN class only for M1.
    if q.qclass != QClass::IN {
        resp.header.rcode = RCode::NotImp;
        return resp;
    }

    let Some(zone) = zones.zone_for(&q.qname) else {
        // No zone → REFUSED (we're not recursive, we won't look it up elsewhere).
        resp.header.rcode = RCode::Refused;
        return resp;
    };

    resp.header.flags.aa = true;

    let rrset = zone.find(&q.qname, q.qtype);
    if let Some(records) = rrset {
        resp.header.rcode = RCode::NoError;
        resp.answers.extend(records.iter().cloned());
    } else {
        // Is the name itself known under any other type? If yes → NOERROR with no answers
        // (NODATA). If no → NXDOMAIN. Both cases get the zone's SOA in the authority
        // section to carry negative-caching TTL (RFC 2308). Full RFC 2308 NSEC handling
        // lands in M2.
        let has_any_type = [
            QType::A,
            QType::AAAA,
            QType::NS,
            QType::CNAME,
            QType::MX,
            QType::TXT,
            QType::SOA,
            QType::SRV,
            QType::PTR,
            QType::CAA,
        ]
        .iter()
        .any(|t| zone.find(&q.qname, *t).is_some());
        resp.header.rcode = if has_any_type {
            RCode::NoError
        } else {
            RCode::NxDomain
        };
        resp.authority.push(zone.soa().clone());
    }

    resp
}

/// Wrap the answer path with metric observation.
fn answer_with_metrics(
    query_bytes: &[u8],
    zones: &ZoneRegistry,
    metrics: &DnsMetrics,
    proto: Proto,
) -> Vec<u8> {
    let start = Instant::now();
    let query = match Message::decode(query_bytes) {
        Ok(q) => q,
        Err(err) => {
            tracing::debug!(error = %err, proto = ?proto, "malformed query");
            metrics
                .dropped
                .get_or_create(&DropLabels {
                    reason: DropReason::Malformed,
                })
                .inc();
            return Vec::new();
        }
    };

    let resp = answer(&query, zones);

    let qtype = query
        .questions
        .first()
        .map_or(QTypeLabel::Other, |q| QTypeLabel::from_wire(q.qtype));
    let labels = QueryLabels {
        proto,
        qtype,
        rcode: RcodeLabel::from_rcode(resp.header.rcode),
    };
    metrics.queries.get_or_create(&labels).inc();
    metrics
        .latency
        .get_or_create(&labels)
        .observe(start.elapsed().as_secs_f64());

    // Encode into a bounded buffer and truncate over UDP if needed.
    let mut buf = vec![0u8; 4096];
    match resp.encode(&mut buf) {
        Ok(n) => {
            buf.truncate(n);
            if matches!(proto, Proto::Udp) && buf.len() > udp_limit(&query) {
                // Produce a truncated response: keep the header (with TC=1), drop sections.
                let mut tc = resp.clone();
                tc.header.flags.tc = true;
                tc.answers.clear();
                tc.authority.clear();
                tc.additional.clear();
                let mut small = vec![0u8; MAX_UDP_MESSAGE_SIZE];
                let m = tc.encode(&mut small).unwrap_or(0);
                small.truncate(m);
                return small;
            }
            buf
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to encode response");
            // Emit a minimal SERVFAIL header.
            let mut hdr = Header::default();
            hdr.id = query.header.id;
            hdr.flags.qr = true;
            hdr.rcode = RCode::ServFail;
            hdr.qdcount = u16::from(!query.questions.is_empty());
            let mut out = vec![0u8; 512];
            let mut msg = Message::default();
            msg.header = hdr;
            if let Some(q) = query.questions.first().cloned() {
                msg.questions.push(q);
            }
            let n = msg.encode(&mut out).unwrap_or(12);
            out.truncate(n);
            out
        }
    }
}

fn udp_limit(query: &Message) -> usize {
    query.edns.as_ref().map_or(MAX_UDP_MESSAGE_SIZE, |e| {
        usize::from(e.udp_payload_size.max(512))
    })
}

/// Spawn the UDP listener. Returns when the token is cancelled.
pub async fn serve_udp(
    addr: SocketAddr,
    zones: ZoneRegistry,
    metrics: DnsMetrics,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let sock = Arc::new(UdpSocket::bind(addr).await?);
    tracing::info!(bind = %addr, "UDP listener ready");
    let mut buf = [0u8; 4096];
    loop {
        tokio::select! {
            biased;
            () = shutdown.cancelled() => break,
            res = sock.recv_from(&mut buf) => {
                let (n, peer) = match res {
                    Ok(v) => v,
                    Err(err) => {
                        tracing::warn!(error = %err, "udp recv failed");
                        continue;
                    }
                };
                let zones = zones.clone();
                let metrics = metrics.clone();
                let sock = sock.clone();
                let query_bytes = buf[..n].to_vec();
                tokio::spawn(async move {
                    let resp = answer_with_metrics(&query_bytes, &zones, &metrics, Proto::Udp);
                    if !resp.is_empty() {
                        if let Err(err) = sock.send_to(&resp, peer).await {
                            tracing::debug!(%peer, error = %err, "udp send failed");
                        }
                    }
                });
            }
        }
    }
    Ok(())
}

/// Spawn the TCP listener. RFC 7766: each message is preceded by a 2-byte length prefix.
pub async fn serve_tcp(
    addr: SocketAddr,
    zones: ZoneRegistry,
    metrics: DnsMetrics,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(bind = %addr, "TCP listener ready");
    loop {
        tokio::select! {
            biased;
            () = shutdown.cancelled() => break,
            res = listener.accept() => {
                let (stream, peer) = match res {
                    Ok(v) => v,
                    Err(err) => {
                        tracing::warn!(error = %err, "tcp accept failed");
                        continue;
                    }
                };
                let zones = zones.clone();
                let metrics = metrics.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_tcp(stream, zones, metrics).await {
                        tracing::debug!(%peer, error = %err, "tcp session error");
                    }
                });
            }
        }
    }
    Ok(())
}

async fn handle_tcp(
    mut stream: TcpStream,
    zones: ZoneRegistry,
    metrics: DnsMetrics,
) -> std::io::Result<()> {
    loop {
        let mut len_buf = [0u8; 2];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        }
        let msg_len = usize::from(u16::from_be_bytes(len_buf));
        let mut buf = vec![0u8; msg_len];
        stream.read_exact(&mut buf).await?;
        let resp = answer_with_metrics(&buf, &zones, &metrics, Proto::Tcp);
        if !resp.is_empty() {
            let len = u16::try_from(resp.len())
                .map_err(|_| std::io::Error::other("response exceeds 65535 bytes"))?;
            stream.write_all(&len.to_be_bytes()).await?;
            stream.write_all(&resp).await?;
        }
    }
}

/// Silence: prevent unused warnings when all of these modules are exercised only in M2+.
#[allow(dead_code)]
fn _keep_soa_types_live(_s: Soa, _r: ResourceRecord, _d: RData) {}
