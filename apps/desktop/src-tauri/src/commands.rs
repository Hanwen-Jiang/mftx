use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mft_core::app_config::AppConfig;
use mft_core::discovery::{discover_for_streaming, local_discoverable_ipv4s, DiscoveryBeacon};
use mft_core::peer::initiator::pull_all;
use mft_core::transfer::{offer_paths_with_progress, DeviceIdentity, ProgressFn};
use mft_protocol::crypto::PasswordRecord;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events::{
    PEER_DISCOVERED, TRANSFER_FAILED, TRANSFER_FINISHED, TRANSFER_PROGRESS, TRANSFER_STARTED,
};
use crate::models::{
    display_path, parse_addr, parse_optional_addr, pathbufs, transfer_id, AppStateDto,
    InboxEntryDto, PeerDto, IncomingTransferDecisionDto, PullRequest, SendPathsRequest,
    SettingsRequest, SetupRequest, TransferDirectionDto, TransferEventDto, TransferProgressDto,
    TransferReportDto, TrustDeviceRequest, TrustedDeviceDto,
};
use crate::runtime::DesktopRuntime;
use crate::compat::DESKTOP_COMPAT_PASSWORD;

#[tauri::command]
pub async fn get_app_state(runtime: State<'_, DesktopRuntime>) -> Result<AppStateDto, String> {
    runtime.load_state().await.map_err(to_message)
}

#[tauri::command]
pub async fn get_default_setup() -> Result<SetupRequest, String> {
    Ok(SetupRequest {
        device_name: hostname_label(),
        password: String::new(),
        base_dir: Some(display_path(AppConfig::default_base_dir().map_err(to_message)?)),
        listen_addr: Some(AppConfig::default_listen_addr().to_string()),
    })
}

#[tauri::command]
pub async fn complete_setup(
    runtime: State<'_, DesktopRuntime>,
    request: SetupRequest,
) -> Result<AppStateDto, String> {
    if request.device_name.trim().is_empty() {
        return Err("device name cannot be empty".to_string());
    }
    let password = request
        .password
        .trim()
        .is_empty()
        .then_some(DESKTOP_COMPAT_PASSWORD)
        .unwrap_or(request.password.trim());

    let base_dir = request
        .base_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let config = AppConfig::new(
        request.device_name.trim().to_string(),
        parse_optional_addr(request.listen_addr).map_err(to_message)?,
        PasswordRecord::create(password).map_err(to_message)?,
        AppConfig::resolve_base_dir(base_dir).await.map_err(to_message)?,
    );
    config.save().await.map_err(to_message)?;
    config.save_location().await.map_err(to_message)?;
    runtime.set_config(config).await.map_err(to_message)
}

#[tauri::command]
pub async fn update_settings(
    runtime: State<'_, DesktopRuntime>,
    request: SettingsRequest,
) -> Result<AppStateDto, String> {
    let mut config = runtime.config().await.map_err(to_message)?;

    if let Some(device_name) = request.device_name {
        if device_name.trim().is_empty() {
            return Err("device name cannot be empty".to_string());
        }
        config.device_name = device_name.trim().to_string();
    }
    if let Some(password) = request.password {
        if !password.is_empty() {
            config.password = PasswordRecord::create(&password).map_err(to_message)?;
        }
    }
    if let Some(listen_addr) = request.listen_addr {
        if !listen_addr.trim().is_empty() {
            config.listen_addr = parse_addr(&listen_addr).map_err(to_message)?;
        }
    }
    if let Some(inbox_dir) = request.inbox_dir {
        if !inbox_dir.trim().is_empty() {
            config.inbox_dir = PathBuf::from(inbox_dir);
        }
    }
    if let Some(share_dir) = request.share_dir {
        if !share_dir.trim().is_empty() {
            config.share_dir = PathBuf::from(share_dir);
        }
    }
    if let Some(received_dir) = request.received_dir {
        if !received_dir.trim().is_empty() {
            config.received_dir = PathBuf::from(received_dir);
        }
    }
    if let Some(discovery_targets) = request.discovery_targets {
        // Normalize: trim, drop blanks, de-duplicate (order-preserving).
        let mut seen = HashSet::new();
        config.discovery_targets = discovery_targets
            .into_iter()
            .map(|target| target.trim().to_string())
            .filter(|target| !target.is_empty() && seen.insert(target.clone()))
            .collect();
    }

    runtime.save_config(config).await.map_err(to_message)
}

#[tauri::command]
pub async fn start_peer(
    app: AppHandle,
    runtime: State<'_, DesktopRuntime>,
) -> Result<AppStateDto, String> {
    runtime.start_peer(app).await.map_err(to_message)
}

#[tauri::command]
pub async fn stop_peer(runtime: State<'_, DesktopRuntime>) -> Result<AppStateDto, String> {
    runtime.stop_peer().await.map_err(to_message)
}

#[tauri::command]
pub async fn list_trusted_devices(
    runtime: State<'_, DesktopRuntime>,
) -> Result<Vec<TrustedDeviceDto>, String> {
    runtime.trusted_devices().await.map_err(to_message)
}

#[tauri::command]
pub async fn untrust_device(
    runtime: State<'_, DesktopRuntime>,
    request: TrustDeviceRequest,
) -> Result<AppStateDto, String> {
    let device_id = request
        .device_id
        .parse()
        .map_err(|error| format!("invalid device id: {error}"))?;
    runtime.untrust_device(device_id).await.map_err(to_message)
}

#[tauri::command]
pub async fn respond_incoming_transfer(
    app: AppHandle,
    runtime: State<'_, DesktopRuntime>,
    decision: IncomingTransferDecisionDto,
) -> Result<(), String> {
    runtime
        .respond_incoming_transfer(app, decision)
        .await
        .map_err(to_message)
}

#[tauri::command]
pub async fn discover_peers(
    app: AppHandle,
    runtime: State<'_, DesktopRuntime>,
    seconds: Option<u64>,
) -> Result<Vec<PeerDto>, String> {
    // When the peer is running, read its own PeerTable instead of opening a
    // second discovery socket. The peer's background loop already probes broadcast
    // + every interface + configured "connected" targets on a single socket that
    // (unlike a co-located second socket on Windows) reliably receives replies, so
    // this surfaces verified, reachable peers and fixes the Windows blind spot.
    if let Some(peers) = runtime.discovered_peers().await {
        for peer in &peers {
            let _ = app.emit(PEER_DISCOVERED, peer.clone());
        }
        return Ok(peers);
    }

    let timeout = Duration::from_secs(seconds.unwrap_or(3).clamp(1, 15));
    let state = runtime.load_state().await.map_err(to_message)?;
    let local_addr = state
        .local_addr
        .as_deref()
        .and_then(|value| value.parse::<SocketAddr>().ok());
    let local_ipv4s = local_discoverable_ipv4s().into_iter().collect::<HashSet<_>>();

    // Stream each newly-seen non-local peer to the UI the instant it answers, so
    // fast devices appear immediately rather than waiting for the whole window;
    // slow devices simply arrive later instead of gating the fast ones.
    let on_peer = |beacon: &DiscoveryBeacon| {
        if is_local_beacon(
            beacon,
            state.local_device_id.as_deref(),
            state.local_session_id.as_deref(),
            local_addr,
            &local_ipv4s,
        ) {
            return;
        }
        let _ = app.emit(PEER_DISCOVERED, PeerDto::from(beacon.clone()));
    };

    let peers = discover_for_streaming(timeout, None, on_peer)
        .await
        .map_err(to_message)?;

    Ok(clean_discovered_peers(
        peers,
        state.local_device_id.as_deref(),
        state.local_session_id.as_deref(),
        state.local_addr.as_deref(),
    )
    .into_iter()
    .map(PeerDto::from)
    .collect())
}

#[tauri::command]
pub async fn send_paths(
    app: AppHandle,
    runtime: State<'_, DesktopRuntime>,
    request: SendPathsRequest,
) -> Result<TransferReportDto, String> {
    let id = transfer_id();
    let paths = pathbufs(request.paths.clone());
    let addr = parse_addr(&request.addr).map_err(to_message)?;
    let peer = request.addr.clone();
    let config = runtime.config().await.map_err(to_message)?;
    let identity = DeviceIdentity {
        device_id: config.device_id,
        device_name: config.device_name,
    };

    emit_transfer(
        &app,
        TRANSFER_STARTED,
        TransferEventDto {
            id: id.clone(),
            direction: TransferDirectionDto::Push,
            peer: peer.clone(),
            paths: request.paths.clone(),
            report: None,
            message: None,
        },
    );

    let progress: ProgressFn = {
        let app = app.clone();
        let id = id.clone();
        let peer = peer.clone();
        Arc::new(move |transferred, total| {
            let _ = app.emit(
                TRANSFER_PROGRESS,
                TransferProgressDto {
                    id: id.clone(),
                    direction: TransferDirectionDto::Push,
                    peer: peer.clone(),
                    transferred,
                    total,
                },
            );
        })
    };

    match offer_paths_with_progress(addr, DESKTOP_COMPAT_PASSWORD, identity, &paths, Some(progress))
        .await
    {
        Ok(report) => {
            let dto = TransferReportDto::from(report);
            emit_transfer(
                &app,
                TRANSFER_FINISHED,
                TransferEventDto {
                    id,
                    direction: TransferDirectionDto::Push,
                    peer,
                    paths: request.paths,
                    report: Some(dto.clone()),
                    message: None,
                },
            );
            Ok(dto)
        }
        Err(error) => {
            let message = error.to_string();
            emit_transfer(
                &app,
                TRANSFER_FAILED,
                TransferEventDto {
                    id,
                    direction: TransferDirectionDto::Push,
                    peer,
                    paths: request.paths,
                    report: None,
                    message: Some(message.clone()),
                },
            );
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn pull_from_peer(
    app: AppHandle,
    runtime: State<'_, DesktopRuntime>,
    request: PullRequest,
) -> Result<TransferReportDto, String> {
    let id = transfer_id();
    let addr = parse_addr(&request.addr).map_err(to_message)?;
    let out_dir = match request.out_dir {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => runtime.default_out_dir().await.map_err(to_message)?,
    };
    let peer = request.addr.clone();
    let path_label = display_path(out_dir.clone());

    emit_transfer(
        &app,
        TRANSFER_STARTED,
        TransferEventDto {
            id: id.clone(),
            direction: TransferDirectionDto::Pull,
            peer: peer.clone(),
            paths: vec![path_label.clone()],
            report: None,
            message: None,
        },
    );

    match pull_all(addr, &request.password, out_dir).await {
        Ok(report) => {
            let dto = TransferReportDto::from(report);
            emit_transfer(
                &app,
                TRANSFER_FINISHED,
                TransferEventDto {
                    id,
                    direction: TransferDirectionDto::Pull,
                    peer,
                    paths: vec![path_label],
                    report: Some(dto.clone()),
                    message: None,
                },
            );
            Ok(dto)
        }
        Err(error) => {
            let message = error.to_string();
            emit_transfer(
                &app,
                TRANSFER_FAILED,
                TransferEventDto {
                    id,
                    direction: TransferDirectionDto::Pull,
                    peer,
                    paths: vec![path_label],
                    report: None,
                    message: Some(message.clone()),
                },
            );
            Err(message)
        }
    }
}

/// "Connect to" a peer by address: persist it as a discovery target so the
/// running peer probes it and the verified peer surfaces in the device list.
/// This replaces blind-send: you never connect to an unverified address.
#[tauri::command]
pub async fn connect_peer(
    runtime: State<'_, DesktopRuntime>,
    addr: String,
) -> Result<AppStateDto, String> {
    if addr.trim().is_empty() {
        return Err("请输入对方地址".to_string());
    }
    runtime.add_discovery_target(addr).await.map_err(to_message)
}

#[tauri::command]
pub async fn list_inbox(runtime: State<'_, DesktopRuntime>) -> Result<Vec<InboxEntryDto>, String> {
    let config = runtime.config().await.map_err(to_message)?;
    Ok(list_dir_entries(&config.inbox_dir))
}

#[tauri::command]
pub async fn open_inbox(runtime: State<'_, DesktopRuntime>) -> Result<(), String> {
    let config = runtime.config().await.map_err(to_message)?;
    open_path(config.inbox_dir).map_err(to_message)
}

#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), String> {
    open_path(PathBuf::from(path)).map_err(to_message)
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(to_message)?;
        window.set_focus().map_err(to_message)?;
    }
    Ok(())
}

pub fn emit_transfer(app: &AppHandle, event: &str, payload: TransferEventDto) {
    let _ = app.emit(event, payload);
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn hostname_label() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "mftx-desktop".to_string())
}

fn clean_discovered_peers(
    peers: Vec<DiscoveryBeacon>,
    local_device_id: Option<&str>,
    local_session_id: Option<&str>,
    local_addr: Option<&str>,
) -> Vec<DiscoveryBeacon> {
    let local_addr = local_addr.and_then(|value| value.parse::<SocketAddr>().ok());
    let local_ipv4s = local_discoverable_ipv4s().into_iter().collect::<HashSet<_>>();
    let mut seen_device_ids = HashSet::new();
    let mut seen_sessions = HashSet::new();
    let mut seen_addrs = HashSet::new();
    let mut cleaned = Vec::new();

    for peer in peers {
        if is_local_beacon(
            &peer,
            local_device_id,
            local_session_id,
            local_addr,
            &local_ipv4s,
        ) {
            continue;
        }
        if !seen_device_ids.insert(peer.device_id) {
            continue;
        }
        if !seen_sessions.insert(peer.session_id) {
            continue;
        }
        if let Some(addr) = peer.observed_addr {
            if !seen_addrs.insert(addr) {
                continue;
            }
        }
        cleaned.push(peer);
    }

    cleaned.sort_by(|a, b| {
        a.device_name
            .cmp(&b.device_name)
            .then_with(|| a.observed_addr.cmp(&b.observed_addr))
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    cleaned
}

fn is_local_observed_addr(addr: SocketAddr, local_ipv4s: &HashSet<Ipv4Addr>) -> bool {
    match addr.ip() {
        IpAddr::V4(ip) => ip.is_loopback() || local_ipv4s.contains(&ip),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}

/// True when a discovered beacon is actually this device (by id, session, or
/// observed address) and must not be shown/streamed as a remote peer.
fn is_local_beacon(
    peer: &DiscoveryBeacon,
    local_device_id: Option<&str>,
    local_session_id: Option<&str>,
    local_addr: Option<SocketAddr>,
    local_ipv4s: &HashSet<Ipv4Addr>,
) -> bool {
    if local_device_id.is_some_and(|device_id| device_id == peer.device_id.to_string()) {
        return true;
    }
    if local_session_id.is_some_and(|session_id| session_id == peer.session_id.to_string()) {
        return true;
    }
    if peer.observed_addr.is_some() && peer.observed_addr == local_addr {
        return true;
    }
    peer.observed_addr
        .is_some_and(|addr| is_local_observed_addr(addr, local_ipv4s))
}

/// Top-level entries of a directory (used for the inbox file list), newest first.
/// In-progress `.part` files are skipped.
fn list_dir_entries(dir: &Path) -> Vec<InboxEntryDto> {
    let mut entries = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".part") {
                continue;
            }
            let metadata = entry.metadata().ok();
            entries.push(InboxEntryDto {
                name,
                path: entry.path().to_string_lossy().to_string(),
                size: metadata.as_ref().map(|meta| meta.len()).unwrap_or(0),
                is_dir: metadata.as_ref().map(|meta| meta.is_dir()).unwrap_or(false),
                modified_ms: metadata
                    .as_ref()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|elapsed| elapsed.as_millis() as i64)
                    .unwrap_or(0),
            });
        }
    }
    entries.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    entries
}

fn open_path(path: PathBuf) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // `start` opens a file with its default program and a folder in Explorer
        // (the empty "" is the window-title argument so the path isn't taken as one).
        std::process::Command::new("cmd")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/c", "start", ""])
            .arg(path)
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }

    Ok(())
}
