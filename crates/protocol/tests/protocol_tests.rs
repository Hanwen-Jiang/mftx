use mft_protocol::crypto::{derive_session_key, PasswordRecord, SessionCipher, SessionRole};
use mft_protocol::frame::Frame;
use mft_protocol::manifest::{EntryKind, Manifest, ManifestEntry};
use mft_protocol::path::clean_relative_path;

#[test]
fn clean_relative_path_rejects_traversal_and_absolute_paths() {
    assert_eq!(
        clean_relative_path("folder/hello.txt").unwrap(),
        "folder/hello.txt"
    );
    assert!(clean_relative_path("../secret.txt").is_err());
    assert!(clean_relative_path("/tmp/secret.txt").is_err());
    assert!(clean_relative_path("folder/../../secret.txt").is_err());
}

#[test]
fn password_record_verifies_without_storing_plaintext() {
    let record = PasswordRecord::create("correct horse battery staple").unwrap();

    assert!(record.verify("correct horse battery staple").unwrap());
    assert!(!record.verify("wrong password").unwrap());
    assert!(!record.key_hex.contains("correct horse battery staple"));
}

#[test]
fn encrypted_frames_roundtrip_and_reject_wrong_password() {
    let record = PasswordRecord::create("fixed-password").unwrap();
    let client_nonce = [7_u8; 32];
    let server_nonce = [9_u8; 32];
    let good_key =
        derive_session_key("fixed-password", &record, client_nonce, server_nonce).unwrap();
    let bad_key =
        derive_session_key("wrong-password", &record, client_nonce, server_nonce).unwrap();

    let mut writer = SessionCipher::new(good_key, SessionRole::Initiator);
    let mut reader = SessionCipher::new(good_key, SessionRole::Responder);
    let mut wrong_reader = SessionCipher::new(bad_key, SessionRole::Responder);

    let sealed = writer
        .seal(&Frame::Auth {
            device_id: None,
            device_name: "windows".to_string(),
        })
        .unwrap();

    assert!(wrong_reader.open(&sealed).is_err());
    assert_eq!(
        reader.open(&sealed).unwrap(),
        Frame::Auth {
            device_id: None,
            device_name: "windows".to_string()
        }
    );
}

#[test]
fn session_directions_do_not_reuse_nonce() {
    // Both peers share the SAME key. The first frame each peer seals must NOT
    // collide on (key, nonce): initiator tx and responder tx are different
    // directions, so identical plaintext must yield different ciphertext, and
    // each side must be able to open the other's frame 0.
    let record = PasswordRecord::create("fixed-password").unwrap();
    let key = derive_session_key("fixed-password", &record, [1_u8; 32], [2_u8; 32]).unwrap();

    let mut initiator = SessionCipher::new(key, SessionRole::Initiator);
    let mut responder = SessionCipher::new(key, SessionRole::Responder);

    let frame = Frame::Done;
    let from_initiator = initiator.seal(&frame).unwrap();
    let from_responder = responder.seal(&frame).unwrap();

    // Same key, same plaintext, same counter (0) — only the direction differs.
    // If the nonce were not domain-separated these would be byte-identical.
    assert_ne!(from_initiator, from_responder);

    // Each side decrypts the other's frame 0 correctly.
    assert_eq!(responder.open(&from_initiator).unwrap(), frame);
    assert_eq!(initiator.open(&from_responder).unwrap(), frame);
}

#[test]
fn manifest_roundtrips_through_frame_encoding() {
    let manifest = Manifest {
        id: uuid::Uuid::new_v4(),
        entries: vec![ManifestEntry {
            path: "docs/readme.txt".to_string(),
            kind: EntryKind::File,
            size: 42,
            modified_unix: Some(1_700_000_000),
        }],
        total_bytes: 42,
    };
    let frame = Frame::Manifest(manifest.clone());
    let encoded = mft_protocol::frame::encode_plain(&frame).unwrap();
    let decoded = mft_protocol::frame::decode_plain(&encoded).unwrap();

    assert_eq!(decoded, frame);
}
