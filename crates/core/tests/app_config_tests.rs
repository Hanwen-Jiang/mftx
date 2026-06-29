use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use mft_core::app_config::AppConfig;
use mft_protocol::crypto::PasswordRecord;
use serde_json::json;

#[test]
fn default_runtime_paths_live_under_one_mftx_home() {
    let base_dir = PathBuf::from("/Users/example/mftx");
    let config = AppConfig::new(
        "Haven-Mac",
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 48151)),
        PasswordRecord::create("pw").unwrap(),
        base_dir.clone(),
    );

    assert_eq!(config.base_dir, base_dir);
    assert_eq!(config.inbox_dir, PathBuf::from("/Users/example/mftx/inbox"));
    assert_eq!(config.share_dir, PathBuf::from("/Users/example/mftx/share"));
    assert_eq!(
        config.received_dir,
        PathBuf::from("/Users/example/mftx/received")
    );
    assert_eq!(
        config.default_share_paths(),
        vec![PathBuf::from("/Users/example/mftx/share")]
    );
    assert!(!config.inbox_dir.to_string_lossy().contains("Downloads"));
}

#[tokio::test]
async fn config_round_trips_inside_the_mftx_home_and_creates_transfer_dirs() {
    let root = tempfile::tempdir().unwrap();
    let base_dir = root.path().join("mftx");
    let config = AppConfig::new(
        "test-peer",
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 48151)),
        PasswordRecord::create("pw").unwrap(),
        base_dir.clone(),
    );

    config.save().await.unwrap();
    let loaded = AppConfig::load_from_base(&base_dir).await.unwrap();

    assert_eq!(loaded.device_name, "test-peer");
    assert_eq!(loaded.device_id, config.device_id);
    assert_eq!(loaded.listen_addr.port(), 48151);
    assert_eq!(loaded.base_dir, base_dir);
    assert!(loaded.password.verify("pw").unwrap());
    assert!(loaded.inbox_dir.is_dir());
    assert!(loaded.share_dir.is_dir());
    assert!(loaded.received_dir.is_dir());
    assert!(loaded.config_path().is_file());
}

#[tokio::test]
async fn legacy_config_without_device_id_is_migrated_on_load() {
    let root = tempfile::tempdir().unwrap();
    let base_dir = root.path().join("mftx");
    tokio::fs::create_dir_all(&base_dir).await.unwrap();
    let config_path = AppConfig::config_path_for_base(&base_dir);
    let password = PasswordRecord::create("pw").unwrap();
    let legacy = json!({
        "device_name": "legacy-peer",
        "listen_addr": "0.0.0.0:48151",
        "password": password,
        "base_dir": base_dir,
        "inbox_dir": root.path().join("mftx").join("inbox"),
        "share_dir": root.path().join("mftx").join("share"),
        "received_dir": root.path().join("mftx").join("received")
    });
    tokio::fs::write(&config_path, serde_json::to_vec_pretty(&legacy).unwrap())
        .await
        .unwrap();

    let loaded = AppConfig::load_from_base(root.path().join("mftx"))
        .await
        .unwrap();
    assert_eq!(loaded.device_name, "legacy-peer");
    assert!(!loaded.device_id.is_nil());

    let migrated: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(config_path).await.unwrap()).unwrap();
    assert_eq!(
        migrated["device_id"].as_str().unwrap(),
        loaded.device_id.to_string()
    );
}
