//! End-to-end integration test: send a real DNS query over UDP/TCP and assert the answer.

use std::net::{Ipv4Addr, SocketAddr};

use nebula_metrics::dns::DnsMetrics;
use nebula_metrics::Metrics;
use nebula_server::dns::{serve_tcp, serve_udp, ZoneRegistry};
use nebula_wire::{Message, Name, QClass, QType, Question, RCode, RData};
use nebula_zone::Zone;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio_util::sync::CancellationToken;

const ZONE_TOML: &str = include_str!("../../../zones/example.com.toml");

async fn spawn_server() -> (SocketAddr, SocketAddr, CancellationToken) {
    let zone = Zone::from_toml(ZONE_TOML).unwrap();
    let zones = ZoneRegistry::new();
    zones.replace([zone]);

    let metrics_handle = Metrics::global();
    let dns_metrics = metrics_handle.with_registry_mut(DnsMetrics::register);

    let shutdown = CancellationToken::new();

    // Bind on an ephemeral port so tests can run in parallel without collision.
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let udp_addr = udp.local_addr().unwrap();
    drop(udp); // release so serve_udp can rebind (bind-rebind race is fine in tests).

    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_addr = tcp.local_addr().unwrap();
    drop(tcp);

    {
        let zones = zones.clone();
        let metrics = dns_metrics.clone();
        let tok = shutdown.clone();
        tokio::spawn(async move { serve_udp(udp_addr, zones, metrics, tok).await });
    }
    {
        let zones = zones.clone();
        let metrics = dns_metrics.clone();
        let tok = shutdown.clone();
        tokio::spawn(async move { serve_tcp(tcp_addr, zones, metrics, tok).await });
    }

    // Small delay to ensure listeners are bound before we probe.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    (udp_addr, tcp_addr, shutdown)
}

fn build_query(id: u16, name: &str, qtype: QType) -> Vec<u8> {
    let mut msg = Message::default();
    msg.header.id = id;
    msg.header.flags.rd = false;
    msg.header.qdcount = 1;
    msg.questions.push(Question {
        qname: Name::from_ascii(name).unwrap(),
        qtype,
        qclass: QClass::IN,
    });
    let mut buf = vec![0u8; 512];
    let n = msg.encode(&mut buf).unwrap();
    buf.truncate(n);
    buf
}

#[tokio::test]
async fn udp_query_returns_a_records() {
    let (udp_addr, _tcp_addr, shutdown) = spawn_server().await;

    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.connect(udp_addr).await.unwrap();

    let q = build_query(0xBEEF, "www.example.com", QType::A);
    sock.send(&q).await.unwrap();

    let mut buf = [0u8; 4096];
    let n = tokio::time::timeout(std::time::Duration::from_secs(2), sock.recv(&mut buf))
        .await
        .expect("timeout")
        .unwrap();

    let resp = Message::decode(&buf[..n]).unwrap();
    assert!(resp.header.flags.qr);
    assert!(resp.header.flags.aa);
    assert_eq!(resp.header.rcode, RCode::NoError);
    assert_eq!(resp.answers.len(), 2);
    let ips: Vec<_> = resp
        .answers
        .iter()
        .filter_map(|rr| match &rr.data {
            RData::A(ip) => Some(*ip),
            _ => None,
        })
        .collect();
    assert!(ips.contains(&Ipv4Addr::new(192, 0, 2, 10)));
    assert!(ips.contains(&Ipv4Addr::new(192, 0, 2, 11)));

    shutdown.cancel();
}

#[tokio::test]
async fn udp_nxdomain_response_includes_soa() {
    let (udp_addr, _tcp_addr, shutdown) = spawn_server().await;

    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.connect(udp_addr).await.unwrap();

    let q = build_query(1, "nope.example.com", QType::A);
    sock.send(&q).await.unwrap();

    let mut buf = [0u8; 4096];
    let n = sock.recv(&mut buf).await.unwrap();
    let resp = Message::decode(&buf[..n]).unwrap();
    assert_eq!(resp.header.rcode, RCode::NxDomain);
    assert_eq!(resp.authority.len(), 1);
    assert!(matches!(resp.authority[0].data, RData::Soa(_)));

    shutdown.cancel();
}

#[tokio::test]
async fn tcp_query_with_length_prefix() {
    let (_udp, tcp_addr, shutdown) = spawn_server().await;

    let mut stream = TcpStream::connect(tcp_addr).await.unwrap();
    let q = build_query(0xCAFE, "www.example.com", QType::A);
    let len = u16::try_from(q.len()).unwrap().to_be_bytes();
    stream.write_all(&len).await.unwrap();
    stream.write_all(&q).await.unwrap();

    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await.unwrap();
    let resp_len = u16::from_be_bytes(len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).await.unwrap();

    let resp = Message::decode(&resp_buf).unwrap();
    assert_eq!(resp.header.rcode, RCode::NoError);
    assert_eq!(resp.answers.len(), 2);

    shutdown.cancel();
}

#[tokio::test]
async fn refused_for_out_of_zone_query() {
    let (udp_addr, _tcp, shutdown) = spawn_server().await;
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.connect(udp_addr).await.unwrap();

    let q = build_query(2, "www.not-in-our-zone.com", QType::A);
    sock.send(&q).await.unwrap();

    let mut buf = [0u8; 4096];
    let n = sock.recv(&mut buf).await.unwrap();
    let resp = Message::decode(&buf[..n]).unwrap();
    assert_eq!(resp.header.rcode, RCode::Refused);
    assert!(!resp.header.flags.aa);

    shutdown.cancel();
}

#[tokio::test]
async fn soa_query_returns_soa_record() {
    let (udp_addr, _tcp, shutdown) = spawn_server().await;
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.connect(udp_addr).await.unwrap();

    let q = build_query(3, "example.com", QType::SOA);
    sock.send(&q).await.unwrap();

    let mut buf = [0u8; 4096];
    let n = sock.recv(&mut buf).await.unwrap();
    let resp = Message::decode(&buf[..n]).unwrap();
    assert_eq!(resp.header.rcode, RCode::NoError);
    assert_eq!(resp.answers.len(), 1);
    assert!(matches!(resp.answers[0].data, RData::Soa(_)));

    shutdown.cancel();
}
