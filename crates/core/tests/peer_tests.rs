use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use mft_core::peer::{
    initiator::{pull_all, push_paths},
    PeerConfig, PeerNode, PeerRecord, PeerTable,
};
use mft_protocol::crypto::PasswordRecord;
use uuid::Uuid;

#[tokio::test]
async fn peer_nodes_push_files_in_both_directions() {
    let a_root = tempfile::tempdir().unwrap();
    let b_root = tempfile::tempdir().unwrap();
    let a_inbox = a_root.path().join("inbox");
    let b_inbox = b_root.path().join("inbox");
    let a_file = a_root.path().join("from-a.txt");
    let b_file = b_root.path().join("from-b.txt");
    tokio::fs::write(&a_file, b"hello from a").await.unwrap();
    tokio::fs::write(&b_file, b"hello from b").await.unwrap();

    let password = PasswordRecord::create("pw").unwrap();
    let peer_a = PeerNode::bind(PeerConfig::for_test(
        "peer-a",
        password.clone(),
        Vec::new(),
        a_inbox.clone(),
    ))
    .await
    .unwrap();
    let peer_b = PeerNode::bind(PeerConfig::for_test(
        "peer-b",
        password,
        Vec::new(),
        b_inbox.clone(),
    ))
    .await
    .unwrap();

    let report = push_paths(peer_b.addr(), "pw", &[a_file.clone()])
        .await
        .unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(
        tokio::fs::read(b_inbox.join("from-a.txt")).await.unwrap(),
        b"hello from a"
    );

    let report = push_paths(peer_a.addr(), "pw", &[b_file.clone()])
        .await
        .unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(
        tokio::fs::read(a_inbox.join("from-b.txt")).await.unwrap(),
        b"hello from b"
    );
}

#[tokio::test]
async fn peer_node_serves_pull_manifest_and_resume() {
    let provider_root = tempfile::tempdir().unwrap();
    let pull_root = tempfile::tempdir().unwrap();
    let shared = provider_root.path().join("shared.txt");
    tokio::fs::write(&shared, b"abcdef").await.unwrap();

    let provider = PeerNode::bind(PeerConfig::for_test(
        "provider",
        PasswordRecord::create("pw").unwrap(),
        vec![shared.clone()],
        provider_root.path().join("inbox"),
    ))
    .await
    .unwrap();

    tokio::fs::write(pull_root.path().join("shared.txt.part"), b"abc")
        .await
        .unwrap();

    let report = pull_all(provider.addr(), "pw", pull_root.path())
        .await
        .unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(report.bytes, 3);
    assert_eq!(
        tokio::fs::read(pull_root.path().join("shared.txt"))
            .await
            .unwrap(),
        b"abcdef"
    );
}

#[test]
fn peer_table_resolves_names_and_rejects_ambiguous_names() {
    let table = PeerTable::default();
    let first = PeerRecord {
        device_id: Uuid::new_v4(),
        device_name: "Lou-Win".to_string(),
        session_id: Uuid::new_v4(),
        addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 48151)),
        capabilities: vec!["receive".to_string(), "push".to_string()],
        version: 1,
        first_seen: Duration::from_secs(1),
        last_seen: Duration::from_secs(1),
    };
    let second = PeerRecord {
        addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 48152)),
        session_id: Uuid::new_v4(),
        ..first.clone()
    };

    table.upsert(first.clone());
    assert_eq!(table.resolve("Lou-Win").unwrap().addr, first.addr);

    table.upsert(second);
    let error = table.resolve("Lou-Win").unwrap_err().to_string();
    assert!(error.contains("multiple peers named Lou-Win"));
}
