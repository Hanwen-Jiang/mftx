use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use mft_core::discovery::discover_for;
use mft_core::discovery::{
    parse_arp_candidate_ips, parse_discovery_targets, parse_local_interface_ipv4s, DiscoveryBeacon,
    DiscoveryMessageKind, DISCOVERY_PORT,
};
use mft_core::fs_manifest::build_manifest;
use mft_protocol::crypto::PasswordRecord;
use mft_protocol::manifest::EntryKind;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn build_manifest_preserves_relative_unicode_paths() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("资料/hello world.txt");
    tokio::fs::create_dir_all(nested.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&nested, b"hello").await.unwrap();

    let manifest = build_manifest(&[dir.path().join("资料")]).await.unwrap();

    assert_eq!(manifest.total_bytes, 5);
    assert!(manifest
        .entries
        .iter()
        .any(|entry| entry.path == "资料/hello world.txt"
            && entry.kind == EntryKind::File
            && entry.size == 5));
}

#[test]
fn discovery_beacon_does_not_include_file_names() {
    let beacon = DiscoveryBeacon::new(
        Uuid::new_v4(),
        "macbook".to_string(),
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 48151)),
        vec!["send".to_string(), "receive".to_string()],
    );
    let encoded = beacon.to_wire().unwrap();
    let decoded = DiscoveryBeacon::from_wire(&encoded).unwrap();

    assert_eq!(decoded.device_name, "macbook");
    assert_eq!(decoded.device_id, beacon.device_id);
    assert_eq!(decoded.port, 48151);
    assert!(!encoded.contains("secret.txt"));
    assert!(!encoded.contains("资料"));
}

#[test]
fn arp_candidate_parser_extracts_neighbor_addresses() {
    let output = r#"
? (192.168.2.1) at aa:bb:cc:dd:ee:ff on en0 ifscope [ethernet]
gateway (192.168.2.31) at 11:22:33:44:55:66 on en0 ifscope [ethernet]
Interface: 192.168.2.44 --- 0x6
  Internet Address      Physical Address      Type
  192.168.2.31          11-22-33-44-55-66     dynamic
  10.0.0.5              aa-bb-cc-dd-ee-00     dynamic
  224.0.0.251           01-00-5e-00-00-fb     static
"#;

    let ips = parse_arp_candidate_ips(output);

    assert_eq!(
        ips,
        vec![
            Ipv4Addr::new(192, 168, 2, 1),
            Ipv4Addr::new(192, 168, 2, 31),
            Ipv4Addr::new(10, 0, 0, 5),
        ]
    );
}

#[test]
fn discovery_targets_default_to_the_discovery_port_when_no_port_is_given() {
    let targets = parse_discovery_targets(["100.64.0.2", "100.64.0.3:49000"]);

    assert_eq!(
        targets,
        vec![
            SocketAddr::new(Ipv4Addr::new(100, 64, 0, 2).into(), DISCOVERY_PORT),
            SocketAddr::new(Ipv4Addr::new(100, 64, 0, 3).into(), 49000),
        ]
    );
}

#[test]
fn discovery_targets_skip_blanks_and_deduplicate() {
    let targets = parse_discovery_targets([
        "  100.64.0.2  ",
        "",
        "100.64.0.2",
        "100.64.0.2:48150",
        "not a valid host name with spaces",
    ]);

    // The trimmed literal and its explicit-port twin collapse to one address;
    // blanks and unparseable entries are dropped instead of failing the set.
    assert_eq!(
        targets,
        vec![SocketAddr::new(
            Ipv4Addr::new(100, 64, 0, 2).into(),
            DISCOVERY_PORT
        )]
    );
}

#[test]
fn arp_parser_extracts_local_interface_addresses() {
    let output = r#"
接口: 192.168.112.154 --- 0x10
  Internet 地址         物理地址              类型
  192.168.112.1         aa-bb-cc-dd-ee-ff     动态
Interface: 10.0.0.12 --- 0x6
  Internet Address      Physical Address      Type
  10.0.0.1              11-22-33-44-55-66     dynamic
Interface: 127.0.0.1 --- 0x1
"#;

    let local_ips = parse_local_interface_ipv4s(output);

    assert_eq!(
        local_ips,
        vec![Ipv4Addr::new(192, 168, 112, 154), Ipv4Addr::new(10, 0, 0, 12)]
    );
}

#[test]
fn discovery_probe_wire_is_marked_and_has_no_transfer_port() {
    let probe = DiscoveryBeacon::probe("mft-discover".to_string());
    let encoded = probe.to_wire().unwrap();
    let decoded = DiscoveryBeacon::from_wire(&encoded).unwrap();

    assert_eq!(decoded.kind, DiscoveryMessageKind::Probe);
    assert!(decoded.is_probe());
    assert_eq!(decoded.port, 0);
}

#[tokio::test]
async fn discovery_can_run_while_another_listener_owns_the_discovery_port() {
    let first = tokio::spawn(async {
        let _ = discover_for(Duration::from_millis(250)).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let second = discover_for(Duration::from_millis(50)).await;

    first.await.unwrap();
    assert!(
        second.is_ok(),
        "second discovery should reuse discovery port"
    );
}

#[tokio::test]
async fn server_rejects_wrong_password() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let source = dir.path().join("file.txt");
    tokio::fs::write(&source, b"content").await.unwrap();

    let password = PasswordRecord::create("right-password").unwrap();
    let server = mft_core::transfer::TransferServer::bind_for_test(
        password,
        vec![source],
        dir.path().join("inbox"),
    )
    .await
    .unwrap();

    let result =
        mft_core::transfer::download_all(server.addr(), "wrong-password", out.path()).await;

    assert!(result.is_err());
    let message = result.unwrap_err().to_string();
    assert!(
        message.contains("authentication failed") || message.contains("password"),
        "unexpected wrong-password error: {message}"
    );
}
