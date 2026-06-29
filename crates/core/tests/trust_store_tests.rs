use mft_core::trust_store::{TrustedDeviceStore, TRUSTED_DEVICES_FILE_NAME};
use uuid::Uuid;

#[tokio::test]
async fn trust_store_adds_updates_and_removes_devices_by_device_id() {
    let root = tempfile::tempdir().unwrap();
    let store = TrustedDeviceStore::new(root.path());
    let device_id = Uuid::new_v4();

    assert!(!store.contains(device_id).await.unwrap());
    let trusted = store
        .trust(device_id, "Lou-Win", 1000)
        .await
        .unwrap();

    assert_eq!(trusted.device_id, device_id);
    assert_eq!(trusted.display_name, "Lou-Win");
    assert_eq!(trusted.first_trusted_at_ms, 1000);
    assert_eq!(trusted.last_seen_at_ms, Some(1000));
    assert!(store.path().ends_with(TRUSTED_DEVICES_FILE_NAME));
    assert!(store.contains(device_id).await.unwrap());

    let updated = store
        .trust(device_id, "Lou-Win-Renamed", 2000)
        .await
        .unwrap();
    assert_eq!(updated.first_trusted_at_ms, 1000);
    assert_eq!(updated.last_seen_at_ms, Some(2000));
    assert_eq!(updated.display_name, "Lou-Win-Renamed");
    assert_eq!(store.list().await.unwrap().len(), 1);

    assert!(store.untrust(device_id).await.unwrap());
    assert!(!store.contains(device_id).await.unwrap());
    assert!(!store.untrust(device_id).await.unwrap());
}
