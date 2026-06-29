use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mft_protocol::crypto::PasswordRecord;
use uuid::Uuid;

use super::limits::TransferLimits;
use crate::transfer::IncomingTransferHandler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptPolicy {
    Always,
    Reject,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwritePolicy {
    Never,
    Always,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerCapabilities {
    pub receive_push: bool,
    pub serve_pull: bool,
    pub resume_pull: bool,
    pub encrypted_frames: bool,
    pub blake3: bool,
    pub protocol_version: u16,
}

impl PeerCapabilities {
    pub fn for_share_paths(has_share_paths: bool) -> Self {
        Self {
            receive_push: true,
            serve_pull: has_share_paths,
            resume_pull: true,
            encrypted_frames: true,
            blake3: true,
            protocol_version: mft_protocol::frame::PROTOCOL_VERSION,
        }
    }

    pub fn to_wire_strings(&self) -> Vec<String> {
        let mut caps = Vec::new();
        if self.receive_push {
            caps.push("receive".to_string());
        }
        caps.push("push".to_string());
        if self.serve_pull {
            caps.push("pull".to_string());
        }
        if self.resume_pull {
            caps.push("resume-pull".to_string());
        }
        if self.encrypted_frames {
            caps.push("encrypted".to_string());
        }
        if self.blake3 {
            caps.push("blake3".to_string());
        }
        caps
    }
}

#[derive(Debug, Clone)]
pub struct PeerConfig {
    pub device_id: Uuid,
    pub device_name: String,
    pub listen_addr: SocketAddr,
    pub discovery_port: u16,
    pub password: PasswordRecord,
    pub inbox_dir: PathBuf,
    pub share_paths: Vec<PathBuf>,
    pub announce_interval: Duration,
    pub peer_ttl: Duration,
    pub accept_policy: AcceptPolicy,
    pub overwrite_policy: OverwritePolicy,
    pub enable_discovery: bool,
    pub capabilities: PeerCapabilities,
    pub limits: TransferLimits,
    pub incoming_handler: Option<Arc<dyn IncomingTransferHandler>>,
}

impl PeerConfig {
    pub fn new(
        device_name: impl Into<String>,
        listen_addr: SocketAddr,
        password: PasswordRecord,
        share_paths: Vec<PathBuf>,
        inbox_dir: PathBuf,
    ) -> Self {
        let capabilities = PeerCapabilities::for_share_paths(!share_paths.is_empty());
        Self {
            device_id: Uuid::new_v4(),
            device_name: device_name.into(),
            listen_addr,
            discovery_port: crate::discovery::DISCOVERY_PORT,
            password,
            inbox_dir,
            share_paths,
            announce_interval: Duration::from_secs(2),
            peer_ttl: Duration::from_secs(15),
            accept_policy: AcceptPolicy::Always,
            overwrite_policy: OverwritePolicy::Never,
            enable_discovery: true,
            capabilities,
            limits: TransferLimits::default(),
            incoming_handler: None,
        }
    }

    pub fn with_device_id(mut self, device_id: Uuid) -> Self {
        self.device_id = device_id;
        self
    }

    pub fn with_incoming_handler(mut self, handler: Arc<dyn IncomingTransferHandler>) -> Self {
        self.incoming_handler = Some(handler);
        self
    }

    pub fn for_test(
        device_name: impl Into<String>,
        password: PasswordRecord,
        share_paths: Vec<PathBuf>,
        inbox_dir: PathBuf,
    ) -> Self {
        let mut config = Self::new(
            device_name,
            "127.0.0.1:0".parse().expect("valid test listen addr"),
            password,
            share_paths,
            inbox_dir,
        );
        config.enable_discovery = false;
        config.announce_interval = Duration::from_millis(250);
        config.peer_ttl = Duration::from_secs(2);
        config
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.device_name.trim().is_empty() {
            anyhow::bail!("peer device name cannot be empty");
        }
        if self.peer_ttl <= self.announce_interval {
            anyhow::bail!("peer ttl must be greater than announce interval");
        }
        Ok(())
    }
}
