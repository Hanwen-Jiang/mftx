use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mft_protocol::crypto::{
    derive_session_key_from_password_and_salt, derive_session_key_from_record, PasswordRecord,
    SessionCipher, SessionRole,
};
use mft_protocol::frame::{decode_plain, encode_plain, Frame, PROTOCOL_VERSION};
use mft_protocol::manifest::{EntryKind, Manifest, ManifestEntry};
use mft_protocol::path::clean_relative_path;
use rand::RngCore;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::fs_manifest::{build_manifest_bundle, ManifestBundle};

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferReport {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub device_id: Uuid,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransferOffer {
    pub offer_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub manifest: Manifest,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransferDecision {
    pub accepted: bool,
    pub message: Option<String>,
}

/// Final result of an accepted incoming transfer, reported to the handler once
/// the bytes have actually been received (or the connection failed mid-stream).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransferOutcome {
    pub success: bool,
    pub files: usize,
    pub bytes: u64,
    pub message: Option<String>,
}

/// Sink for live send-side byte progress: `(transferred, total)`.
pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

pub trait IncomingTransferHandler: Send + Sync + std::fmt::Debug + 'static {
    fn decide<'a>(
        &'a self,
        offer: IncomingTransferOffer,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = IncomingTransferDecision> + Send + 'a>>;

    /// Called once an offer is accepted, right before its bytes are received.
    /// Default no-op so existing handlers (CLI/tests) need no changes.
    fn on_start(&self, _offer_id: Uuid, _peer: &str, _files: usize, _bytes: u64) {}

    /// Called periodically while receiving, with cumulative bytes written and
    /// the offer's total. Throttled by the caller (~1% / 1 MiB granularity).
    fn on_progress(&self, _offer_id: Uuid, _transferred: u64, _total: u64) {}

    /// Called once the accepted transfer finishes (success) or aborts (failure).
    fn on_complete(&self, _offer_id: Uuid, _outcome: IncomingTransferOutcome) {}
}

#[derive(Debug)]
pub struct TransferServer {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl TransferServer {
    pub async fn bind_for_test(
        password: PasswordRecord,
        share_paths: Vec<PathBuf>,
        inbox_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        Self::bind(("127.0.0.1:0").parse()?, password, share_paths, inbox_dir).await
    }

    pub async fn bind(
        bind_addr: SocketAddr,
        password: PasswordRecord,
        share_paths: Vec<PathBuf>,
        inbox_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        Self::bind_with_handler(bind_addr, password, share_paths, inbox_dir, None).await
    }

    pub async fn bind_with_handler(
        bind_addr: SocketAddr,
        password: PasswordRecord,
        share_paths: Vec<PathBuf>,
        inbox_dir: PathBuf,
        incoming_handler: Option<Arc<dyn IncomingTransferHandler>>,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(bind_addr).await?;
        let addr = listener.local_addr()?;
        let bundle = build_manifest_bundle(&share_paths).await?;
        fs::create_dir_all(&inbox_dir).await?;

        let state = Arc::new(ServerState {
            password,
            manifest: bundle.manifest,
            files: bundle.files,
            inbox_dir,
            incoming_handler,
        });
        let task = tokio::spawn(server_loop(listener, state));

        Ok(Self { addr, task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for TransferServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Debug)]
struct ServerState {
    password: PasswordRecord,
    manifest: Manifest,
    files: HashMap<String, PathBuf>,
    inbox_dir: PathBuf,
    incoming_handler: Option<Arc<dyn IncomingTransferHandler>>,
}

async fn server_loop(listener: TcpListener, state: Arc<ServerState>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            break;
        };
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let _ = handle_connection(stream, state).await;
        });
    }
}

/// An accepted offer currently being received, used to report progress/outcome.
struct ActiveOffer {
    offer_id: Uuid,
    files: usize,
    bytes: u64,
}

/// Throttled bridge from the receive loop to `IncomingTransferHandler::on_progress`.
struct ProgressHook<'a> {
    handler: &'a dyn IncomingTransferHandler,
    offer_id: Uuid,
    total: u64,
    received: &'a mut u64,
    last_reported: &'a mut u64,
}

impl ProgressHook<'_> {
    fn bump(&mut self, delta: u64) {
        *self.received += delta;
        // Report at ~1% granularity (and never finer than one chunk) to keep the
        // event rate bounded regardless of transfer size.
        let threshold = (self.total / 100).max(CHUNK_BYTES as u64);
        if *self.received - *self.last_reported >= threshold || *self.received >= self.total {
            *self.last_reported = *self.received;
            self.handler.on_progress(self.offer_id, *self.received, self.total);
        }
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<ServerState>) -> anyhow::Result<()> {
    let (mut cipher, _peer_identity) =
        server_handshake(&mut stream, &state.password, state.manifest.clone()).await?;

    // When an incoming handler is installed (e.g. the desktop trust prompt), the
    // receiver MUST NOT write to the inbox until a TransferOffer has been
    // accepted, and may then only write paths that were part of that accepted
    // offer. Without a handler the legacy (CLI/compat) push behavior is kept.
    let require_offer = state.incoming_handler.is_some();
    let mut accepted_paths: Option<HashSet<String>> = None;
    let mut active: Option<ActiveOffer> = None;
    let mut received: u64 = 0;
    let mut last_reported: u64 = 0;
    let mut completed = false;

    let result = loop {
        let frame = match read_encrypted_frame(&mut stream, &mut cipher).await {
            Ok(frame) => frame,
            Err(_) => break Ok(()),
        };

        match frame {
            Frame::TransferOffer {
                offer_id,
                device_id,
                device_name,
                manifest,
                files,
                bytes,
            } => {
                // One offer per connection. A second offer would orphan the
                // first offer's completion callback (leaving the receiver UI
                // stuck "running" and leaking its `active_peers` entry — a slow
                // DoS vector), so reject it and tear the connection down. The
                // post-loop branch then reports the first offer as failed.
                if active.is_some() {
                    let _ = write_encrypted_frame(
                        &mut stream,
                        &mut cipher,
                        &Frame::Error {
                            code: "offer-already-active".to_string(),
                            message: "a transfer offer is already in progress on this connection"
                                .to_string(),
                        },
                    )
                    .await;
                    break Ok(());
                }
                let offered_paths = manifest
                    .entries
                    .iter()
                    .filter_map(|entry| clean_relative_path(&entry.path).ok())
                    .collect::<HashSet<String>>();
                let peer = device_name.clone();
                let decision = decide_incoming_transfer(
                    &state,
                    IncomingTransferOffer {
                        offer_id,
                        device_id,
                        device_name,
                        manifest,
                        files,
                        bytes,
                    },
                )
                .await;
                if let Err(error) = write_encrypted_frame(
                    &mut stream,
                    &mut cipher,
                    &Frame::TransferDecision {
                        offer_id,
                        accepted: decision.accepted,
                        message: decision.message.clone(),
                    },
                )
                .await
                {
                    break Err(error);
                }
                if !decision.accepted {
                    break Ok(());
                }
                accepted_paths = Some(offered_paths);
                received = 0;
                last_reported = 0;
                // Announce the accepted transfer (covers both the prompt path and
                // pre-trusted auto-accept) before any bytes flow.
                if let Some(handler) = &state.incoming_handler {
                    handler.on_start(offer_id, &peer, files, bytes);
                }
                active = Some(ActiveOffer {
                    offer_id,
                    files,
                    bytes,
                });
            }
            Frame::GetFile { path, offset } => {
                if let Err(error) = send_file(&mut stream, &mut cipher, &state, &path, offset).await
                {
                    break Err(error);
                }
            }
            Frame::PutFileStart { entry } => {
                if require_offer && !upload_is_allowed(accepted_paths.as_ref(), &entry.path) {
                    let _ = write_encrypted_frame(
                        &mut stream,
                        &mut cipher,
                        &Frame::Error {
                            code: "offer-required".to_string(),
                            message: "upload requires an accepted transfer offer".to_string(),
                        },
                    )
                    .await;
                    break Ok(());
                }
                let ack_path = entry.path.clone();
                let mut hook = match (&state.incoming_handler, &active) {
                    (Some(handler), Some(offer)) => Some(ProgressHook {
                        handler: handler.as_ref(),
                        offer_id: offer.offer_id,
                        total: offer.bytes,
                        received: &mut received,
                        last_reported: &mut last_reported,
                    }),
                    _ => None,
                };
                if let Err(error) =
                    receive_upload(&mut stream, &mut cipher, &state.inbox_dir, entry, hook.as_mut())
                        .await
                {
                    break Err(error);
                }
                if let Err(error) =
                    write_encrypted_frame(&mut stream, &mut cipher, &Frame::Ack { path: ack_path })
                        .await
                {
                    break Err(error);
                }
            }
            Frame::Done => {
                if let (Some(handler), Some(offer)) = (&state.incoming_handler, &active) {
                    handler.on_progress(offer.offer_id, received, offer.bytes);
                    handler.on_complete(
                        offer.offer_id,
                        IncomingTransferOutcome {
                            success: true,
                            files: offer.files,
                            bytes: received,
                            message: None,
                        },
                    );
                    completed = true;
                }
                break Ok(());
            }
            _ => {
                if let Err(error) = write_encrypted_frame(
                    &mut stream,
                    &mut cipher,
                    &Frame::Error {
                        code: "unexpected-frame".to_string(),
                        message: "unexpected frame in transfer loop".to_string(),
                    },
                )
                .await
                {
                    break Err(error);
                }
            }
        }
    };

    // An accepted offer that never reached `Done` (connection dropped, integrity
    // failure, etc.) must be reported as failed so the receiver UI stays truthful
    // instead of showing a stale "完成".
    if !completed {
        if let (Some(handler), Some(offer)) = (&state.incoming_handler, &active) {
            let message = match &result {
                Ok(()) => "transfer ended before completion".to_string(),
                Err(error) => error.to_string(),
            };
            handler.on_complete(
                offer.offer_id,
                IncomingTransferOutcome {
                    success: false,
                    files: offer.files,
                    bytes: received,
                    message: Some(message),
                },
            );
        }
    }

    result
}

async fn server_handshake(
    stream: &mut TcpStream,
    record: &PasswordRecord,
    manifest: Manifest,
) -> anyhow::Result<(SessionCipher, Option<DeviceIdentity>)> {
    let hello = read_plain_frame(stream).await?;
    let Frame::Hello {
        device_id,
        device_name,
        version,
        client_nonce,
    } = hello
    else {
        anyhow::bail!("expected hello");
    };
    if version != PROTOCOL_VERSION {
        anyhow::bail!("unsupported protocol version {version}");
    }

    let mut server_nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut server_nonce);
    write_plain_frame(
        stream,
        &Frame::HelloAck {
            device_id: None,
            device_name: local_device_name(),
            version: PROTOCOL_VERSION,
            server_nonce,
            password_salt_hex: record.salt_hex.clone(),
        },
    )
    .await?;

    let key = derive_session_key_from_record(record, client_nonce, server_nonce)?;
    let mut cipher = SessionCipher::new(key, SessionRole::Responder);
    match read_encrypted_frame(stream, &mut cipher).await? {
        Frame::Auth {
            device_id: auth_device_id,
            device_name: auth_device_name,
        } => {
            write_encrypted_frame(stream, &mut cipher, &Frame::AuthOk { manifest }).await?;
            let identity_device_id = auth_device_id.or(device_id);
            let identity_name = if auth_device_name.trim().is_empty() {
                device_name
            } else {
                auth_device_name
            };
            Ok((
                cipher,
                identity_device_id.map(|device_id| DeviceIdentity {
                    device_id,
                    device_name: identity_name,
                }),
            ))
        }
        _ => anyhow::bail!("expected auth"),
    }
}

async fn decide_incoming_transfer(
    state: &ServerState,
    offer: IncomingTransferOffer,
) -> IncomingTransferDecision {
    match &state.incoming_handler {
        Some(handler) => handler.decide(offer).await,
        None => IncomingTransferDecision {
            accepted: true,
            message: None,
        },
    }
}

async fn client_handshake(
    stream: &mut TcpStream,
    password: &str,
) -> anyhow::Result<(SessionCipher, Manifest)> {
    let (cipher, manifest, _) = client_handshake_with_identity(stream, password, None).await?;
    Ok((cipher, manifest))
}

async fn client_handshake_with_identity(
    stream: &mut TcpStream,
    password: &str,
    identity: Option<&DeviceIdentity>,
) -> anyhow::Result<(SessionCipher, Manifest, Option<DeviceIdentity>)> {
    let mut client_nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut client_nonce);
    write_plain_frame(
        stream,
        &Frame::Hello {
            device_id: identity.map(|identity| identity.device_id),
            device_name: identity
                .map(|identity| identity.device_name.clone())
                .unwrap_or_else(local_device_name),
            version: PROTOCOL_VERSION,
            client_nonce,
        },
    )
    .await?;

    let ack = read_plain_frame(stream).await?;
    let Frame::HelloAck {
        device_id,
        device_name,
        version,
        server_nonce,
        password_salt_hex,
    } = ack
    else {
        anyhow::bail!("expected hello ack");
    };
    if version != PROTOCOL_VERSION {
        anyhow::bail!("unsupported protocol version {version}");
    }

    let key = derive_session_key_from_password_and_salt(
        password,
        &password_salt_hex,
        client_nonce,
        server_nonce,
    )?;
    let mut cipher = SessionCipher::new(key, SessionRole::Initiator);
    write_encrypted_frame(
        stream,
        &mut cipher,
        &Frame::Auth {
            device_id: identity.map(|identity| identity.device_id),
            device_name: identity
                .map(|identity| identity.device_name.clone())
                .unwrap_or_else(local_device_name),
        },
    )
    .await?;

    let peer_identity = device_id.map(|device_id| DeviceIdentity {
        device_id,
        device_name,
    });
    match read_encrypted_frame(stream, &mut cipher).await {
        Ok(Frame::AuthOk { manifest }) => Ok((cipher, manifest, peer_identity)),
        Ok(Frame::Error { message, .. }) => anyhow::bail!(message),
        Ok(_) => anyhow::bail!("expected auth ok"),
        Err(error) => anyhow::bail!("authentication failed; check password ({error})"),
    }
}

pub async fn download_all(
    addr: SocketAddr,
    password: &str,
    out_dir: impl AsRef<Path>,
) -> anyhow::Result<TransferReport> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir).await?;
    let mut stream = connect_with_timeout(addr).await?;
    let (mut cipher, manifest) = client_handshake(&mut stream, password).await?;
    let mut report = TransferReport { files: 0, bytes: 0 };

    for entry in manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Directory)
    {
        let path = safe_join(out_dir, &entry.path)?;
        fs::create_dir_all(path).await?;
    }

    for entry in manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::File)
    {
        let dest = safe_join(out_dir, &entry.path)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }
        let part = part_path(&dest)?;
        let offset = match fs::metadata(&part).await {
            Ok(metadata) if metadata.len() <= entry.size => metadata.len(),
            _ => 0,
        };
        if offset == 0 {
            let _ = fs::remove_file(&part).await;
        }

        write_encrypted_frame(
            &mut stream,
            &mut cipher,
            &Frame::GetFile {
                path: entry.path.clone(),
                offset,
            },
        )
        .await?;

        let mut output = OpenOptions::new()
            .create(true)
            .append(offset > 0)
            .write(true)
            .truncate(offset == 0)
            .open(&part)
            .await?;

        let done = loop {
            match read_encrypted_frame(&mut stream, &mut cipher).await? {
                Frame::FileChunk {
                    path,
                    offset: _,
                    data,
                    last: _,
                } if path == entry.path => {
                    report.bytes += data.len() as u64;
                    output.write_all(&data).await?;
                }
                Frame::FileDone {
                    path,
                    size,
                    blake3_hex,
                } if path == entry.path => break (size, blake3_hex),
                Frame::Error { message, .. } => anyhow::bail!(message),
                other => anyhow::bail!("unexpected frame while downloading: {other:?}"),
            }
        };
        output.flush().await?;
        drop(output);

        let actual = blake3_file(&part).await?;
        if done.0 != entry.size || actual != done.1 {
            anyhow::bail!("integrity check failed for {}", entry.path);
        }
        replace_file(&part, &dest).await?;
        report.files += 1;
    }

    write_encrypted_frame(&mut stream, &mut cipher, &Frame::Done).await?;
    Ok(report)
}

pub async fn upload_paths(
    addr: SocketAddr,
    password: &str,
    paths: &[PathBuf],
) -> anyhow::Result<TransferReport> {
    let ManifestBundle { manifest, files } = build_manifest_bundle(paths).await?;
    let mut stream = connect_with_timeout(addr).await?;
    let (mut cipher, _) = client_handshake(&mut stream, password).await?;
    let mut report = TransferReport { files: 0, bytes: 0 };

    for entry in &manifest.entries {
        write_encrypted_frame(
            &mut stream,
            &mut cipher,
            &Frame::PutFileStart {
                entry: entry.clone(),
            },
        )
        .await?;
        if entry.kind == EntryKind::File {
            let source = files
                .get(&entry.path)
                .ok_or_else(|| anyhow::anyhow!("missing source for {}", entry.path))?;
            send_file_contents(&mut stream, &mut cipher, source, entry, None).await?;
            report.files += 1;
            report.bytes += entry.size;
        }
        match read_encrypted_frame(&mut stream, &mut cipher).await? {
            Frame::Ack { path } if path == entry.path => {}
            Frame::Error { message, .. } => anyhow::bail!(message),
            other => anyhow::bail!("unexpected frame while waiting for upload ack: {other:?}"),
        }
    }

    write_encrypted_frame(&mut stream, &mut cipher, &Frame::Done).await?;
    Ok(report)
}

pub async fn offer_paths(
    addr: SocketAddr,
    password: &str,
    identity: DeviceIdentity,
    paths: &[PathBuf],
) -> anyhow::Result<TransferReport> {
    offer_paths_with_progress(addr, password, identity, paths, None).await
}

pub async fn offer_paths_with_progress(
    addr: SocketAddr,
    password: &str,
    identity: DeviceIdentity,
    paths: &[PathBuf],
    progress: Option<ProgressFn>,
) -> anyhow::Result<TransferReport> {
    let ManifestBundle { manifest, files } = build_manifest_bundle(paths).await?;
    let mut stream = connect_with_timeout(addr).await?;
    let (mut cipher, _, _) =
        client_handshake_with_identity(&mut stream, password, Some(&identity)).await?;
    let offer_id = Uuid::new_v4();
    write_encrypted_frame(
        &mut stream,
        &mut cipher,
        &Frame::TransferOffer {
            offer_id,
            device_id: identity.device_id,
            device_name: identity.device_name,
            files: manifest
                .entries
                .iter()
                .filter(|entry| entry.kind == EntryKind::File)
                .count(),
            bytes: manifest.total_bytes,
            manifest: manifest.clone(),
        },
    )
    .await?;

    match read_encrypted_frame(&mut stream, &mut cipher).await? {
        Frame::TransferDecision {
            offer_id: decision_id,
            accepted: true,
            ..
        } if decision_id == offer_id => {}
        Frame::TransferDecision {
            offer_id: decision_id,
            accepted: false,
            message,
        } if decision_id == offer_id => {
            anyhow::bail!(message.unwrap_or_else(|| "transfer rejected".to_string()));
        }
        Frame::Error { message, .. } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected frame while waiting for transfer decision: {other:?}"),
    }

    upload_manifest(&mut stream, &mut cipher, &manifest, &files, progress.as_ref()).await
}

/// Throttled bridge from the send loop to a caller-provided `ProgressFn`.
struct SendProgress<'a> {
    sink: &'a ProgressFn,
    total: u64,
    sent: &'a mut u64,
    last_reported: &'a mut u64,
}

impl SendProgress<'_> {
    fn bump(&mut self, delta: u64) {
        *self.sent += delta;
        let threshold = (self.total / 100).max(CHUNK_BYTES as u64);
        if *self.sent - *self.last_reported >= threshold || *self.sent >= self.total {
            *self.last_reported = *self.sent;
            (self.sink)(*self.sent, self.total);
        }
    }
}

async fn upload_manifest(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    manifest: &Manifest,
    files: &HashMap<String, PathBuf>,
    progress: Option<&ProgressFn>,
) -> anyhow::Result<TransferReport> {
    let mut report = TransferReport { files: 0, bytes: 0 };
    let total = manifest.total_bytes;
    let mut sent: u64 = 0;
    let mut last_reported: u64 = 0;

    for entry in &manifest.entries {
        write_encrypted_frame(
            stream,
            cipher,
            &Frame::PutFileStart {
                entry: entry.clone(),
            },
        )
        .await?;
        if entry.kind == EntryKind::File {
            let source = files
                .get(&entry.path)
                .ok_or_else(|| anyhow::anyhow!("missing source for {}", entry.path))?;
            let mut hook = progress.map(|sink| SendProgress {
                sink,
                total,
                sent: &mut sent,
                last_reported: &mut last_reported,
            });
            send_file_contents(stream, cipher, source, entry, hook.as_mut()).await?;
            report.files += 1;
            report.bytes += entry.size;
        }
        match read_encrypted_frame(stream, cipher).await? {
            Frame::Ack { path } if path == entry.path => {}
            Frame::Error { message, .. } => anyhow::bail!(message),
            other => anyhow::bail!("unexpected frame while waiting for upload ack: {other:?}"),
        }
    }

    write_encrypted_frame(stream, cipher, &Frame::Done).await?;
    Ok(report)
}

async fn send_file(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    state: &ServerState,
    requested_path: &str,
    offset: u64,
) -> anyhow::Result<()> {
    let clean = clean_relative_path(requested_path)?;
    let source = state
        .files
        .get(&clean)
        .ok_or_else(|| anyhow::anyhow!("file is not shared: {clean}"))?;
    let entry = state
        .manifest
        .entries
        .iter()
        .find(|entry| entry.path == clean && entry.kind == EntryKind::File)
        .ok_or_else(|| anyhow::anyhow!("manifest entry missing: {clean}"))?;
    if offset > entry.size {
        anyhow::bail!("resume offset exceeds file size for {clean}");
    }
    send_file_contents_from_offset(stream, cipher, source, entry, offset, None).await
}

async fn send_file_contents(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    source: &Path,
    entry: &ManifestEntry,
    progress: Option<&mut SendProgress<'_>>,
) -> anyhow::Result<()> {
    send_file_contents_from_offset(stream, cipher, source, entry, 0, progress).await
}

async fn send_file_contents_from_offset(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    source: &Path,
    entry: &ManifestEntry,
    offset: u64,
    mut progress: Option<&mut SendProgress<'_>>,
) -> anyhow::Result<()> {
    let mut file = File::open(source).await?;
    if offset > 0 {
        file.seek(SeekFrom::Start(offset)).await?;
    }
    let mut current = offset;
    let mut buf = vec![0_u8; CHUNK_BYTES];
    loop {
        let read = file.read(&mut buf).await?;
        if read == 0 {
            break;
        }
        let next = current + read as u64;
        write_encrypted_frame(
            stream,
            cipher,
            &Frame::FileChunk {
                path: entry.path.clone(),
                offset: current,
                data: buf[..read].to_vec(),
                last: next == entry.size,
            },
        )
        .await?;
        current = next;
        if let Some(hook) = progress.as_deref_mut() {
            hook.bump(read as u64);
        }
    }
    write_encrypted_frame(
        stream,
        cipher,
        &Frame::FileDone {
            path: entry.path.clone(),
            size: entry.size,
            blake3_hex: blake3_file(source).await?,
        },
    )
    .await
}

async fn receive_upload(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    inbox_dir: &Path,
    entry: ManifestEntry,
    mut progress: Option<&mut ProgressHook<'_>>,
) -> anyhow::Result<()> {
    let dest = safe_join(inbox_dir, &entry.path)?;
    if entry.kind == EntryKind::Directory {
        fs::create_dir_all(dest).await?;
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    let part = part_path(&dest)?;
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&part)
        .await?;
    let mut hasher = blake3::Hasher::new();
    let mut written = 0_u64;

    loop {
        match read_encrypted_frame(stream, cipher).await? {
            Frame::FileChunk { path, data, .. } if path == entry.path => {
                written += data.len() as u64;
                hasher.update(&data);
                output.write_all(&data).await?;
                if let Some(hook) = progress.as_deref_mut() {
                    hook.bump(data.len() as u64);
                }
            }
            Frame::FileDone {
                path,
                size,
                blake3_hex,
            } if path == entry.path => {
                output.flush().await?;
                drop(output);
                if size != entry.size
                    || written != entry.size
                    || hasher.finalize().to_hex().as_str() != blake3_hex
                {
                    anyhow::bail!("integrity check failed for upload {}", entry.path);
                }
                replace_file(&part, &dest).await?;
                return Ok(());
            }
            Frame::Error { message, .. } => anyhow::bail!(message),
            other => anyhow::bail!("unexpected frame while uploading: {other:?}"),
        }
    }
}

async fn write_plain_frame(stream: &mut TcpStream, frame: &Frame) -> anyhow::Result<()> {
    write_len_prefixed(stream, &encode_plain(frame)?).await
}

async fn read_plain_frame(stream: &mut TcpStream) -> anyhow::Result<Frame> {
    let bytes = read_len_prefixed(stream).await?;
    Ok(decode_plain(&bytes)?)
}

async fn write_encrypted_frame(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    frame: &Frame,
) -> anyhow::Result<()> {
    let sealed = cipher.seal(frame)?;
    write_len_prefixed(stream, &sealed).await
}

async fn read_encrypted_frame(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
) -> anyhow::Result<Frame> {
    let sealed = read_len_prefixed(stream).await?;
    Ok(cipher.open(&sealed)?)
}

async fn write_len_prefixed(stream: &mut TcpStream, bytes: &[u8]) -> anyhow::Result<()> {
    if bytes.len() > MAX_FRAME_BYTES {
        anyhow::bail!("frame too large: {} bytes", bytes.len());
    }
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_len_prefixed(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let len = stream.read_u32().await? as usize;
    if len > MAX_FRAME_BYTES {
        anyhow::bail!("frame too large: {len} bytes");
    }
    let mut bytes = vec![0_u8; len];
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}

fn safe_join(base: &Path, relative: &str) -> anyhow::Result<PathBuf> {
    let clean = clean_relative_path(relative)?;
    Ok(base.join(clean))
}

/// A `PutFileStart` is only honored once an offer was accepted AND the requested
/// path was part of that accepted offer's manifest. Returns false when no offer
/// has been accepted or the path was never offered.
fn upload_is_allowed(accepted_paths: Option<&HashSet<String>>, requested: &str) -> bool {
    match (accepted_paths, clean_relative_path(requested)) {
        (Some(paths), Ok(clean)) => paths.contains(&clean),
        _ => false,
    }
}

/// Atomically move `from` onto `to`, overwriting any existing file.
///
/// `fs::rename` overwrites on Unix but FAILS on Windows when the destination
/// already exists, which made every re-send of a same-named file abort the
/// transfer (the sender saw an error and the file never landed). Try the rename
/// first (fast path; also covers the common first-write case where `to` is
/// absent) and, only if it fails, remove the destination and retry.
async fn replace_file(from: &Path, to: &Path) -> anyhow::Result<()> {
    if fs::rename(from, to).await.is_ok() {
        return Ok(());
    }
    let _ = fs::remove_file(to).await;
    fs::rename(from, to).await?;
    Ok(())
}

fn part_path(dest: &Path) -> anyhow::Result<PathBuf> {
    let file_name = dest
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("destination has no file name: {}", dest.display()))?;
    Ok(dest.with_file_name(format!("{file_name}.part")))
}

async fn blake3_file(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path).await?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0_u8; CHUNK_BYTES];
    loop {
        let read = file.read(&mut buf).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Connect with a bounded timeout so an unreachable address fails fast instead
/// of blocking on the OS connect timeout (~20s on Windows) and hanging the UI.
async fn connect_with_timeout(addr: SocketAddr) -> anyhow::Result<TcpStream> {
    const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
    match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) => Err(error.into()),
        Err(_) => anyhow::bail!("connection to {addr} timed out — host not reachable"),
    }
}

fn local_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "mft-device".to_string())
}
