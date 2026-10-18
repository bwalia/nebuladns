//! End-to-end: SIGHUP reloads zones in the running `nebuladns` binary instead of killing it.
#![cfg(unix)]

use std::net::{Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use nebula_wire::{Message, Name, QClass, QType, Question, RData};

/// Kills the server and removes the temp dir even when an assertion fails.
struct Server {
    child: Child,
    dir: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn zone_toml(ip: &str) -> String {
    format!(
        r#"origin = "reload.test."
default_ttl = 60

[soa]
mname = "ns1.reload.test."
rname = "hostmaster.reload.test."
serial = 1
refresh = 3600
retry = 600
expire = 86400
minimum = 60

[[records]]
name = "@"
type = "NS"
value = "ns1.reload.test."

[[records]]
name = "www"
type = "A"
value = "{ip}"
"#
    )
}

fn spawn(dir: &Path, dns: SocketAddr) -> Server {
    let config = format!(
        r#"[api]
bind = "127.0.0.1:{api}"

[metrics]
bind = "127.0.0.1:{metrics}"

[logging]
filter = "warn"
json = false

[dns]
udp = "{dns}"

[[zones]]
file = "reload.test.toml"
"#,
        api = free_port(),
        metrics = free_port(),
    );
    std::fs::write(dir.join("nebuladns.toml"), config).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_nebuladns"))
        .arg("--config")
        .arg(dir.join("nebuladns.toml"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    Server {
        child,
        dir: dir.to_path_buf(),
    }
}

fn query_www(dns: SocketAddr) -> Option<Ipv4Addr> {
    let mut msg = Message::default();
    msg.header.id = 0x5151;
    msg.header.qdcount = 1;
    msg.questions.push(Question {
        qname: Name::from_ascii("www.reload.test").unwrap(),
        qtype: QType::A,
        qclass: QClass::IN,
    });
    let mut buf = vec![0u8; 512];
    let n = msg.encode(&mut buf).unwrap();

    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    sock.send_to(&buf[..n], dns).ok()?;
    let mut resp = [0u8; 4096];
    let n = sock.recv(&mut resp).ok()?;
    Message::decode(&resp[..n])
        .ok()?
        .answers
        .iter()
        .find_map(|rr| match &rr.data {
            RData::A(ip) => Some(*ip),
            _ => None,
        })
}

fn wait_for_answer(dns: SocketAddr, want: Ipv4Addr) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = None;
    while Instant::now() < deadline {
        last = query_www(dns);
        if last == Some(want) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("expected www.reload.test A {want}, last answer {last:?}");
}

fn sighup(server: &Server) {
    let status = Command::new("kill")
        .arg("-HUP")
        .arg(server.child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn sighup_reloads_zones_and_keeps_serving() {
    let dir = std::env::temp_dir().join(format!("nebuladns-sighup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let zone = dir.join("reload.test.toml");
    std::fs::write(&zone, zone_toml("192.0.2.1")).unwrap();

    let dns: SocketAddr = format!("127.0.0.1:{}", free_port()).parse().unwrap();
    let mut server = spawn(&dir, dns);
    wait_for_answer(dns, Ipv4Addr::new(192, 0, 2, 1));

    // A good edit is picked up without a restart.
    std::fs::write(&zone, zone_toml("192.0.2.2")).unwrap();
    sighup(&server);
    wait_for_answer(dns, Ipv4Addr::new(192, 0, 2, 2));
    assert!(
        server.child.try_wait().unwrap().is_none(),
        "server exited on SIGHUP"
    );

    // A broken edit is rejected and the previous zones keep serving.
    std::fs::write(&zone, "this is not a zone").unwrap();
    sighup(&server);
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        server.child.try_wait().unwrap().is_none(),
        "server exited on bad reload"
    );
    assert_eq!(query_www(dns), Some(Ipv4Addr::new(192, 0, 2, 2)));
}
