use std::future::Future;
use std::pin::Pin;

use mft_core::transfer::{
    download_all, offer_paths, offer_paths_with_progress, upload_paths, DeviceIdentity,
    IncomingTransferDecision, IncomingTransferHandler, IncomingTransferOffer,
    IncomingTransferOutcome, ProgressFn, TransferServer,
};
use mft_protocol::crypto::PasswordRecord;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct StaticIncomingDecision {
    accepted: bool,
}

impl IncomingTransferHandler for StaticIncomingDecision {
    fn decide<'a>(
        &'a self,
        _offer: IncomingTransferOffer,
    ) -> Pin<Box<dyn Future<Output = IncomingTransferDecision> + Send + 'a>> {
        Box::pin(async move {
            IncomingTransferDecision {
                accepted: self.accepted,
                message: if self.accepted {
                    None
                } else {
                    Some("rejected in test".to_string())
                },
            }
        })
    }
}

#[tokio::test]
async fn downloads_file_and_resumes_partial_file() {
    let source_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("big.txt");
    tokio::fs::write(&source, b"abcdef").await.unwrap();

    let password = PasswordRecord::create("pw").unwrap();
    let server = TransferServer::bind_for_test(
        password,
        vec![source.clone()],
        source_dir.path().join("inbox"),
    )
    .await
    .unwrap();

    let partial = out_dir.path().join("big.txt.part");
    tokio::fs::write(&partial, b"abc").await.unwrap();

    let report = download_all(server.addr(), "pw", out_dir.path())
        .await
        .unwrap();

    assert_eq!(report.files, 1);
    assert_eq!(report.bytes, 3);
    assert_eq!(
        tokio::fs::read(out_dir.path().join("big.txt"))
            .await
            .unwrap(),
        b"abcdef"
    );
}

#[tokio::test]
async fn uploads_directory_to_server_inbox() {
    let upload_dir = tempfile::tempdir().unwrap();
    let server_dir = tempfile::tempdir().unwrap();
    let nested = upload_dir.path().join("folder/note.txt");
    tokio::fs::create_dir_all(nested.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&nested, b"from windows").await.unwrap();

    let password = PasswordRecord::create("pw").unwrap();
    let inbox = server_dir.path().join("inbox");
    let server = TransferServer::bind_for_test(password, Vec::new(), inbox.clone())
        .await
        .unwrap();

    let report = upload_paths(server.addr(), "pw", &[upload_dir.path().join("folder")])
        .await
        .unwrap();

    assert_eq!(report.files, 1);
    assert_eq!(
        tokio::fs::read(inbox.join("folder/note.txt"))
            .await
            .unwrap(),
        b"from windows"
    );
}

#[tokio::test]
async fn transfer_offer_accepts_before_writing_to_inbox() {
    let upload_dir = tempfile::tempdir().unwrap();
    let server_dir = tempfile::tempdir().unwrap();
    let source = upload_dir.path().join("offered.txt");
    tokio::fs::write(&source, b"offered").await.unwrap();

    let password = PasswordRecord::create("pw").unwrap();
    let inbox = server_dir.path().join("inbox");
    let server = TransferServer::bind_with_handler(
        "127.0.0.1:0".parse().unwrap(),
        password,
        Vec::new(),
        inbox.clone(),
        Some(std::sync::Arc::new(StaticIncomingDecision { accepted: true })),
    )
    .await
    .unwrap();

    let report = offer_paths(
        server.addr(),
        "pw",
        DeviceIdentity {
            device_id: Uuid::new_v4(),
            device_name: "sender".to_string(),
        },
        &[source],
    )
    .await
    .unwrap();

    assert_eq!(report.files, 1);
    assert_eq!(
        tokio::fs::read(inbox.join("offered.txt")).await.unwrap(),
        b"offered"
    );
}

#[tokio::test]
async fn transfer_offer_rejects_without_writing_to_inbox() {
    let upload_dir = tempfile::tempdir().unwrap();
    let server_dir = tempfile::tempdir().unwrap();
    let source = upload_dir.path().join("rejected.txt");
    tokio::fs::write(&source, b"rejected").await.unwrap();

    let password = PasswordRecord::create("pw").unwrap();
    let inbox = server_dir.path().join("inbox");
    let server = TransferServer::bind_with_handler(
        "127.0.0.1:0".parse().unwrap(),
        password,
        Vec::new(),
        inbox.clone(),
        Some(std::sync::Arc::new(StaticIncomingDecision { accepted: false })),
    )
    .await
    .unwrap();

    let result = offer_paths(
        server.addr(),
        "pw",
        DeviceIdentity {
            device_id: Uuid::new_v4(),
            device_name: "sender".to_string(),
        },
        &[source],
    )
    .await;

    assert!(result.unwrap_err().to_string().contains("rejected"));
    assert!(!inbox.join("rejected.txt").exists());
}

#[tokio::test]
async fn direct_upload_without_offer_is_rejected_when_handler_present() {
    let upload_dir = tempfile::tempdir().unwrap();
    let server_dir = tempfile::tempdir().unwrap();
    let source = upload_dir.path().join("sneaky.txt");
    tokio::fs::write(&source, b"sneaky").await.unwrap();

    let password = PasswordRecord::create("pw").unwrap();
    let inbox = server_dir.path().join("inbox");
    // Handler present => the decision gate is mandatory. A peer that skips the
    // TransferOffer and pushes a file directly (legacy upload_paths) must be
    // refused, and nothing may land in the inbox.
    let server = TransferServer::bind_with_handler(
        "127.0.0.1:0".parse().unwrap(),
        password,
        Vec::new(),
        inbox.clone(),
        Some(std::sync::Arc::new(StaticIncomingDecision { accepted: true })),
    )
    .await
    .unwrap();

    let result = upload_paths(server.addr(), "pw", &[source]).await;

    assert!(result.is_err());
    assert!(!inbox.join("sneaky.txt").exists());
}

#[derive(Debug, Default)]
struct RecordingHandler {
    started: std::sync::Mutex<Option<(usize, u64)>>,
    progress_max: std::sync::Mutex<u64>,
    progress_total: std::sync::Mutex<u64>,
    completed: std::sync::Mutex<Option<(bool, usize, u64)>>,
}

impl IncomingTransferHandler for RecordingHandler {
    fn decide<'a>(
        &'a self,
        _offer: IncomingTransferOffer,
    ) -> Pin<Box<dyn Future<Output = IncomingTransferDecision> + Send + 'a>> {
        Box::pin(async move {
            IncomingTransferDecision {
                accepted: true,
                message: None,
            }
        })
    }

    fn on_start(&self, _offer_id: Uuid, _peer: &str, files: usize, bytes: u64) {
        *self.started.lock().unwrap() = Some((files, bytes));
    }

    fn on_progress(&self, _offer_id: Uuid, transferred: u64, total: u64) {
        let mut max = self.progress_max.lock().unwrap();
        *max = (*max).max(transferred);
        *self.progress_total.lock().unwrap() = total;
    }

    fn on_complete(&self, _offer_id: Uuid, outcome: IncomingTransferOutcome) {
        *self.completed.lock().unwrap() = Some((outcome.success, outcome.files, outcome.bytes));
    }
}

#[tokio::test]
async fn reports_start_progress_and_completion_to_handler() {
    let upload_dir = tempfile::tempdir().unwrap();
    let server_dir = tempfile::tempdir().unwrap();
    let source = upload_dir.path().join("payload.bin");
    let payload = vec![7_u8; 5 * 1024 * 1024]; // 5 MiB → several chunks
    tokio::fs::write(&source, &payload).await.unwrap();
    let total = payload.len() as u64;

    let password = PasswordRecord::create("pw").unwrap();
    let inbox = server_dir.path().join("inbox");
    let handler = std::sync::Arc::new(RecordingHandler::default());
    let server = TransferServer::bind_with_handler(
        "127.0.0.1:0".parse().unwrap(),
        password,
        Vec::new(),
        inbox.clone(),
        Some(handler.clone()),
    )
    .await
    .unwrap();

    // Record send-side progress too.
    let sent_seen = std::sync::Arc::new(std::sync::Mutex::new(0_u64));
    let sent_total = std::sync::Arc::new(std::sync::Mutex::new(0_u64));
    let progress: ProgressFn = {
        let seen = sent_seen.clone();
        let tot = sent_total.clone();
        std::sync::Arc::new(move |transferred, total| {
            let mut s = seen.lock().unwrap();
            *s = (*s).max(transferred);
            *tot.lock().unwrap() = total;
        })
    };

    let report = offer_paths_with_progress(
        server.addr(),
        "pw",
        DeviceIdentity {
            device_id: Uuid::new_v4(),
            device_name: "sender".to_string(),
        },
        &[source],
        Some(progress),
    )
    .await
    .unwrap();

    assert_eq!(report.files, 1);
    assert_eq!(report.bytes, total);

    // on_start fires before any bytes flow, so it is set by the time we return.
    assert_eq!(*handler.started.lock().unwrap(), Some((1, total)));
    // The send side observed progress up to the full total.
    assert_eq!(*sent_total.lock().unwrap(), total);
    assert_eq!(*sent_seen.lock().unwrap(), total);

    // on_complete is emitted by the server after it receives Done, which can race
    // the sender returning — poll briefly for the truthful completion.
    let mut completed = None;
    for _ in 0..100 {
        if let Some(value) = *handler.completed.lock().unwrap() {
            completed = Some(value);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(completed, Some((true, 1, total)));
    assert_eq!(*handler.progress_total.lock().unwrap(), total);
    assert_eq!(*handler.progress_max.lock().unwrap(), total);
}

#[tokio::test]
async fn resending_same_filename_overwrites_instead_of_failing() {
    // Regression: on Windows `fs::rename` fails when the destination exists, so
    // the second send of a same-named file aborted the transfer (sender error +
    // missing/partial file). Re-sending must overwrite the inbox copy cleanly.
    let upload_dir = tempfile::tempdir().unwrap();
    let server_dir = tempfile::tempdir().unwrap();
    let source = upload_dir.path().join("doc.txt");

    let password = PasswordRecord::create("pw").unwrap();
    let inbox = server_dir.path().join("inbox");
    let server = TransferServer::bind_with_handler(
        "127.0.0.1:0".parse().unwrap(),
        password,
        Vec::new(),
        inbox.clone(),
        Some(std::sync::Arc::new(StaticIncomingDecision { accepted: true })),
    )
    .await
    .unwrap();

    let identity = || DeviceIdentity {
        device_id: Uuid::new_v4(),
        device_name: "sender".to_string(),
    };

    tokio::fs::write(&source, b"first").await.unwrap();
    offer_paths(server.addr(), "pw", identity(), &[source.clone()])
        .await
        .unwrap();
    assert_eq!(tokio::fs::read(inbox.join("doc.txt")).await.unwrap(), b"first");

    // Second send of the same filename — must succeed and replace the contents.
    tokio::fs::write(&source, b"second-and-longer").await.unwrap();
    offer_paths(server.addr(), "pw", identity(), &[source])
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(inbox.join("doc.txt")).await.unwrap(),
        b"second-and-longer"
    );
}
