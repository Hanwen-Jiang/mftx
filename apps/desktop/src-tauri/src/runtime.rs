use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mft_core::app_config::AppConfig;
use mft_core::discovery::{parse_discovery_targets, set_discovery_targets};
use mft_core::peer::{PeerConfig, PeerNode};
use mft_core::transfer::{
    IncomingTransferDecision, IncomingTransferHandler, IncomingTransferOffer,
    IncomingTransferOutcome,
};
use mft_core::trust_store::TrustedDeviceStore;
use mft_protocol::crypto::PasswordRecord;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use crate::events::{
    TRANSFER_FAILED, TRANSFER_FINISHED, TRANSFER_INCOMING_EXPIRED, TRANSFER_INCOMING_REQUESTED,
    TRANSFER_PROGRESS, TRANSFER_STARTED, TRUST_CHANGED,
};
use crate::compat::DESKTOP_COMPAT_PASSWORD;
use crate::models::{
    AppConfigDto, AppStateDto, IncomingTransferDecisionDto, IncomingTransferRequestDto, PeerDto,
    TransferDirectionDto, TransferEventDto, TransferProgressDto, TransferReportDto, TrustedDeviceDto,
};

#[derive(Debug, Clone, Default)]
pub struct DesktopRuntime {
    inner: Arc<Mutex<RuntimeInner>>,
}

#[derive(Debug, Default)]
struct RuntimeInner {
    config: Option<AppConfig>,
    node: Option<PeerNode>,
    pending_incoming: HashMap<Uuid, PendingIncoming>,
}

#[derive(Debug)]
struct PendingIncoming {
    offer: IncomingTransferOffer,
    responder: oneshot::Sender<IncomingTransferDecision>,
}

#[derive(Clone)]
struct IncomingBridge {
    app: AppHandle,
    runtime: DesktopRuntime,
    /// offer_id → peer name, so progress/outcome events (which the core reports
    /// without the name) can be labeled. Sync mutex: critical sections are tiny
    /// and never held across an await.
    active_peers: Arc<std::sync::Mutex<HashMap<Uuid, String>>>,
}

impl fmt::Debug for IncomingBridge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("IncomingBridge").finish_non_exhaustive()
    }
}

impl DesktopRuntime {
    pub async fn load_state(&self) -> anyhow::Result<AppStateDto> {
        let config = AppConfig::load(None).await.ok();

        let mut inner = self.inner.lock().await;
        if let Some(config) = config {
            apply_discovery_targets(&config);
            inner.config = Some(config);
        }

        Self::dto_from_inner(&inner).await
    }

    pub async fn set_config(&self, config: AppConfig) -> anyhow::Result<AppStateDto> {
        apply_discovery_targets(&config);
        let mut inner = self.inner.lock().await;
        inner.config = Some(config);
        Self::dto_from_inner(&inner).await
    }

    pub async fn config(&self) -> anyhow::Result<AppConfig> {
        let mut inner = self.inner.lock().await;
        if let Some(config) = inner.config.clone() {
            return Ok(config);
        }

        let config = AppConfig::load(None).await?;
        apply_discovery_targets(&config);
        inner.config = Some(config.clone());
        Ok(config)
    }

    pub async fn save_config(&self, config: AppConfig) -> anyhow::Result<AppStateDto> {
        config.save().await?;
        config.save_location().await?;

        apply_discovery_targets(&config);
        let mut inner = self.inner.lock().await;
        inner.config = Some(config);
        Self::dto_from_inner(&inner).await
    }

    pub async fn start_peer(&self, app: AppHandle) -> anyhow::Result<AppStateDto> {
        let config = self.config().await?;
        let handler = Arc::new(IncomingBridge {
            app,
            runtime: self.clone(),
            active_peers: Arc::new(std::sync::Mutex::new(HashMap::new())),
        });
        let node = PeerNode::bind(
            PeerConfig::new(
                config.device_name.clone(),
                config.listen_addr,
                PasswordRecord::create(DESKTOP_COMPAT_PASSWORD)?,
                config.default_share_paths(),
                config.inbox_dir.clone(),
            )
            .with_device_id(config.device_id)
            .with_incoming_handler(handler),
        )
        .await?;

        let mut inner = self.inner.lock().await;
        inner.node = Some(node);
        Self::dto_from_inner(&inner).await
    }

    pub async fn stop_peer(&self) -> anyhow::Result<AppStateDto> {
        let mut inner = self.inner.lock().await;
        inner.node.take();
        inner.pending_incoming.clear();
        Self::dto_from_inner(&inner).await
    }

    pub async fn default_out_dir(&self) -> anyhow::Result<PathBuf> {
        // Merged inbox: pulled files land in the same inbox as pushed files, so
        // there is a single place to look for everything received.
        Ok(self.config().await?.inbox_dir)
    }

    /// Live discovered peers from the running peer's own PeerTable. The node's
    /// background discovery loop maintains this on a single non-contending socket,
    /// so reading it (instead of opening a second discovery socket) both works on
    /// Windows and only surfaces peers that actually responded — i.e. verified,
    /// reachable devices. Returns `None` when the peer is stopped.
    pub async fn discovered_peers(&self) -> Option<Vec<PeerDto>> {
        let inner = self.inner.lock().await;
        let node = inner.node.as_ref()?;
        let local_session = node.session_id();
        let local_device = inner.config.as_ref().map(|config| config.device_id);
        Some(
            node.peer_table()
                .list()
                .into_iter()
                .filter(|record| {
                    record.session_id != local_session && Some(record.device_id) != local_device
                })
                .map(PeerDto::from)
                .collect(),
        )
    }

    /// Append a "connect" target (a peer address the user typed) to the persisted
    /// discovery targets and apply it immediately, so the running peer starts
    /// probing it and the verified peer shows up in the device list.
    pub async fn add_discovery_target(&self, target: String) -> anyhow::Result<AppStateDto> {
        let mut config = self.config().await?;
        let target = target.trim().to_string();
        if !target.is_empty() && !config.discovery_targets.iter().any(|existing| existing == &target)
        {
            config.discovery_targets.push(target);
        }
        self.save_config(config).await
    }

    pub async fn trusted_devices(&self) -> anyhow::Result<Vec<TrustedDeviceDto>> {
        let config = self.config().await?;
        let store = TrustedDeviceStore::new(&config.base_dir);
        Ok(store
            .list()
            .await?
            .into_iter()
            .map(TrustedDeviceDto::from)
            .collect())
    }

    pub async fn untrust_device(&self, device_id: Uuid) -> anyhow::Result<AppStateDto> {
        let config = self.config().await?;
        let store = TrustedDeviceStore::new(&config.base_dir);
        let _ = store.untrust(device_id).await?;
        let inner = self.inner.lock().await;
        Self::dto_from_inner(&inner).await
    }

    pub async fn respond_incoming_transfer(
        &self,
        app: AppHandle,
        decision: IncomingTransferDecisionDto,
    ) -> anyhow::Result<()> {
        let request_id: Uuid = decision.id.parse()?;
        let pending = {
            let mut inner = self.inner.lock().await;
            inner.pending_incoming.remove(&request_id)
        }
        .ok_or_else(|| anyhow::anyhow!("incoming transfer request not found"))?;

        if decision.accepted && decision.trust_device {
            let config = self.config().await?;
            TrustedDeviceStore::new(&config.base_dir)
                .trust(pending.offer.device_id, pending.offer.device_name.clone(), now_ms())
                .await?;
        }

        // On accept, the real transfer (start → progress → finish/fail) is
        // reported by the receive loop through the handler callbacks below, so we
        // emit nothing here — emitting "finished" now would be a lie if the bytes
        // never land. On reject, the loop never starts, so surface it directly.
        if !decision.accepted {
            let _ = app.emit(
                TRANSFER_FAILED,
                TransferEventDto {
                    id: request_id.to_string(),
                    direction: TransferDirectionDto::Incoming,
                    peer: pending.offer.device_name.clone(),
                    paths: pending
                        .offer
                        .manifest
                        .entries
                        .iter()
                        .take(5)
                        .map(|entry| entry.path.clone())
                        .collect(),
                    report: None,
                    message: Some("rejected by user".to_string()),
                },
            );
        }

        let _ = pending.responder.send(IncomingTransferDecision {
            accepted: decision.accepted,
            message: if decision.accepted {
                None
            } else {
                Some("rejected by user".to_string())
            },
        });

        Ok(())
    }

    async fn dto_from_inner(inner: &RuntimeInner) -> anyhow::Result<AppStateDto> {
        let trusted_devices = match &inner.config {
            Some(config) => TrustedDeviceStore::new(&config.base_dir)
                .list()
                .await?
                .into_iter()
                .map(TrustedDeviceDto::from)
                .collect(),
            None => Vec::new(),
        };

        Ok(AppStateDto {
            setup_complete: inner.config.is_some(),
            peer_running: inner.node.is_some(),
            local_addr: inner.node.as_ref().map(|node| node.addr().to_string()),
            local_device_id: inner.config.as_ref().map(|config| config.device_id.to_string()),
            local_session_id: inner
                .node
                .as_ref()
                .map(|node| node.session_id().to_string()),
            trusted_devices,
            config: inner.config.as_ref().map(AppConfigDto::from),
        })
    }
}

impl IncomingBridge {
    async fn decide_inner(&self, offer: IncomingTransferOffer) -> IncomingTransferDecision {
        let config = match self.runtime.config().await {
            Ok(config) => config,
            Err(error) => {
                return IncomingTransferDecision {
                    accepted: false,
                    message: Some(error.to_string()),
                };
            }
        };
        let store = TrustedDeviceStore::new(&config.base_dir);
        if matches!(store.contains(offer.device_id).await, Ok(true)) {
            let _ = store
                .trust(offer.device_id, offer.device_name.clone(), now_ms())
                .await;
            let _ = self.app.emit(TRUST_CHANGED, ());
            return IncomingTransferDecision {
                accepted: true,
                message: None,
            };
        }

        let request_id = offer.offer_id;
        let (tx, rx) = oneshot::channel();
        {
            let mut inner = self.runtime.inner.lock().await;
            inner.pending_incoming.insert(
                request_id,
                PendingIncoming {
                    offer: offer.clone(),
                    responder: tx,
                },
            );
        }

        let dto = IncomingTransferRequestDto {
            id: request_id.to_string(),
            device_id: offer.device_id.to_string(),
            device_name: offer.device_name.clone(),
            files: offer.files,
            bytes: offer.bytes,
            paths_preview: offer
                .manifest
                .entries
                .iter()
                .take(5)
                .map(|entry| entry.path.clone())
                .collect(),
            created_at_ms: now_ms(),
        };
        let _ = self.app.emit(TRANSFER_INCOMING_REQUESTED, dto.clone());

        match tokio::time::timeout(Duration::from_secs(120), rx).await {
            Ok(Ok(decision)) => {
                let _ = self.app.emit(TRUST_CHANGED, ());
                decision
            }
            _ => {
                let mut inner = self.runtime.inner.lock().await;
                inner.pending_incoming.remove(&request_id);
                let _ = self.app.emit(TRANSFER_INCOMING_EXPIRED, dto);
                IncomingTransferDecision {
                    accepted: false,
                    message: Some("request expired".to_string()),
                }
            }
        }
    }
}

impl IncomingTransferHandler for IncomingBridge {
    fn decide<'a>(
        &'a self,
        offer: IncomingTransferOffer,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = IncomingTransferDecision> + Send + 'a>>
    {
        Box::pin(async move { self.decide_inner(offer).await })
    }

    fn on_start(&self, offer_id: Uuid, peer: &str, files: usize, bytes: u64) {
        if let Ok(mut map) = self.active_peers.lock() {
            map.insert(offer_id, peer.to_string());
        }
        let _ = self.app.emit(
            TRANSFER_STARTED,
            TransferEventDto {
                id: offer_id.to_string(),
                direction: TransferDirectionDto::Incoming,
                peer: peer.to_string(),
                paths: Vec::new(),
                report: Some(TransferReportDto { files, bytes }),
                message: None,
            },
        );
    }

    fn on_progress(&self, offer_id: Uuid, transferred: u64, total: u64) {
        let peer = self
            .active_peers
            .lock()
            .ok()
            .and_then(|map| map.get(&offer_id).cloned())
            .unwrap_or_default();
        let _ = self.app.emit(
            TRANSFER_PROGRESS,
            TransferProgressDto {
                id: offer_id.to_string(),
                direction: TransferDirectionDto::Incoming,
                peer,
                transferred,
                total,
            },
        );
    }

    fn on_complete(&self, offer_id: Uuid, outcome: IncomingTransferOutcome) {
        let peer = self
            .active_peers
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&offer_id))
            .unwrap_or_default();
        let _ = self.app.emit(
            if outcome.success {
                TRANSFER_FINISHED
            } else {
                TRANSFER_FAILED
            },
            TransferEventDto {
                id: offer_id.to_string(),
                direction: TransferDirectionDto::Incoming,
                peer,
                paths: Vec::new(),
                report: Some(TransferReportDto {
                    files: outcome.files,
                    bytes: outcome.bytes,
                }),
                message: outcome.message,
            },
        );
    }
}

/// Push the config's discovery targets into the core's process-wide set so the
/// next discovery sweep unicast-probes them (Tailscale peers, etc.).
fn apply_discovery_targets(config: &AppConfig) {
    set_discovery_targets(parse_discovery_targets(&config.discovery_targets));
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}
