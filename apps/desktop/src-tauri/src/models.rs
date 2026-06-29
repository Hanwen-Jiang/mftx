use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use mft_core::app_config::AppConfig;
use mft_core::discovery::DiscoveryBeacon;
use mft_core::peer::PeerRecord;
use mft_core::transfer::TransferReport;
use mft_core::trust_store::TrustedDevice;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDirsDto {
    pub base_dir: String,
    pub inbox_dir: String,
    pub share_dir: String,
    pub received_dir: String,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigDto {
    pub device_id: String,
    pub device_name: String,
    pub listen_addr: String,
    pub discovery_targets: Vec<String>,
    pub dirs: AppDirsDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    pub setup_complete: bool,
    pub peer_running: bool,
    pub local_addr: Option<String>,
    pub local_device_id: Option<String>,
    pub local_session_id: Option<String>,
    pub trusted_devices: Vec<TrustedDeviceDto>,
    pub config: Option<AppConfigDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupRequest {
    pub device_name: String,
    pub password: String,
    pub base_dir: Option<String>,
    pub listen_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsRequest {
    pub device_name: Option<String>,
    pub password: Option<String>,
    pub listen_addr: Option<String>,
    pub inbox_dir: Option<String>,
    pub share_dir: Option<String>,
    pub received_dir: Option<String>,
    pub discovery_targets: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerDto {
    pub device_id: String,
    pub device_name: String,
    pub session_id: String,
    pub addr: Option<String>,
    pub port: u16,
    pub capabilities: Vec<String>,
    pub version: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedDeviceDto {
    pub device_id: String,
    pub display_name: String,
    pub first_trusted_at_ms: i64,
    pub last_seen_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustDeviceRequest {
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTransferRequestDto {
    pub id: String,
    pub device_id: String,
    pub device_name: String,
    pub files: usize,
    pub bytes: u64,
    pub paths_preview: Vec<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTransferDecisionDto {
    pub id: String,
    pub accepted: bool,
    pub trust_device: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendPathsRequest {
    pub addr: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub addr: String,
    pub password: String,
    pub out_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferReportDto {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxEntryDto {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferEventDto {
    pub id: String,
    pub direction: TransferDirectionDto,
    pub peer: String,
    pub paths: Vec<String>,
    pub report: Option<TransferReportDto>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgressDto {
    pub id: String,
    pub direction: TransferDirectionDto,
    pub peer: String,
    pub transferred: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferDirectionDto {
    Push,
    Pull,
    Incoming,
}

impl From<&AppConfig> for AppConfigDto {
    fn from(config: &AppConfig) -> Self {
        Self {
            device_id: config.device_id.to_string(),
            device_name: config.device_name.clone(),
            listen_addr: config.listen_addr.to_string(),
            discovery_targets: config.discovery_targets.clone(),
            dirs: AppDirsDto {
                base_dir: display_path(&config.base_dir),
                inbox_dir: display_path(&config.inbox_dir),
                share_dir: display_path(&config.share_dir),
                received_dir: display_path(&config.received_dir),
                config_path: display_path(config.config_path()),
            },
        }
    }
}

impl From<DiscoveryBeacon> for PeerDto {
    fn from(peer: DiscoveryBeacon) -> Self {
        Self {
            device_id: peer.device_id.to_string(),
            device_name: peer.device_name,
            session_id: peer.session_id.to_string(),
            addr: peer.observed_addr.map(|addr| addr.to_string()),
            port: peer.port,
            capabilities: peer.capabilities,
            version: peer.version,
        }
    }
}

impl From<PeerRecord> for PeerDto {
    fn from(peer: PeerRecord) -> Self {
        Self {
            device_id: peer.device_id.to_string(),
            device_name: peer.device_name,
            session_id: peer.session_id.to_string(),
            addr: Some(peer.addr.to_string()),
            port: peer.addr.port(),
            capabilities: peer.capabilities,
            version: peer.version,
        }
    }
}

impl From<TransferReport> for TransferReportDto {
    fn from(report: TransferReport) -> Self {
        Self {
            files: report.files,
            bytes: report.bytes,
        }
    }
}

impl From<TrustedDevice> for TrustedDeviceDto {
    fn from(device: TrustedDevice) -> Self {
        Self {
            device_id: device.device_id.to_string(),
            display_name: device.display_name,
            first_trusted_at_ms: device.first_trusted_at_ms,
            last_seen_at_ms: device.last_seen_at_ms,
        }
    }
}

pub fn pathbufs(paths: Vec<String>) -> Vec<PathBuf> {
    paths.into_iter().map(PathBuf::from).collect()
}

pub fn parse_addr(addr: &str) -> anyhow::Result<SocketAddr> {
    addr.parse()
        .map_err(|error| anyhow::anyhow!("invalid peer address {addr}: {error}"))
}

pub fn parse_optional_addr(addr: Option<String>) -> anyhow::Result<SocketAddr> {
    match addr {
        Some(value) if !value.trim().is_empty() => parse_addr(&value),
        _ => Ok(AppConfig::default_listen_addr()),
    }
}

pub fn transfer_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}
