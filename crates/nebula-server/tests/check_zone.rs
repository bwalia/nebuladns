//! `nebuladns --check-zone`: the Ansible `validate:` gate for zone deploys.

use std::process::Command;

fn check_zone(path: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nebuladns"))
        .arg("--check-zone")
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn accepts_a_valid_zone() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../zones/example.com.toml");
    let out = check_zone(&path);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "ok: example.com."
    );
}

#[test]
fn rejects_an_invalid_or_missing_zone() {
    let bad =
        std::env::temp_dir().join(format!("nebuladns-check-zone-{}.toml", std::process::id()));
    std::fs::write(&bad, "this is not a zone").unwrap();
    let out = check_zone(&bad);
    let _ = std::fs::remove_file(&bad);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("parse zone file"));

    assert!(!check_zone(std::path::Path::new("/nonexistent/zone.toml"))
        .status
        .success());
}
