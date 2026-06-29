use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;

use crate::discovery::{broadcast_once, discover_for_with_responder, DiscoveryBeacon};

use super::table::{PeerRecord, PeerTable};

#[derive(Debug)]
pub struct DiscoveryService {
    announce_task: JoinHandle<()>,
    listen_task: JoinHandle<()>,
}

impl DiscoveryService {
    pub fn spawn(
        beacon: DiscoveryBeacon,
        peer_table: Arc<PeerTable>,
        announce_interval: Duration,
    ) -> Self {
        let announce_beacon = beacon.clone();
        let announce_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(announce_interval);
            loop {
                interval.tick().await;
                let _ = broadcast_once(&announce_beacon).await;
            }
        });

        let own_session_id = beacon.session_id;
        let listen_task = tokio::spawn(async move {
            loop {
                let Ok(peers) =
                    discover_for_with_responder(Duration::from_secs(2), Some(&beacon)).await
                else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                };
                let now = duration_since_epoch();
                for peer in peers {
                    if peer.session_id == own_session_id {
                        continue;
                    }
                    let Some(addr) = peer.observed_addr else {
                        continue;
                    };
                    peer_table.upsert(PeerRecord {
                        device_id: peer.device_id,
                        device_name: peer.device_name,
                        session_id: peer.session_id,
                        addr,
                        capabilities: peer.capabilities,
                        version: peer.version,
                        first_seen: now,
                        last_seen: now,
                    });
                }
            }
        });

        Self {
            announce_task,
            listen_task,
        }
    }
}

impl Drop for DiscoveryService {
    fn drop(&mut self) {
        self.announce_task.abort();
        self.listen_task.abort();
    }
}

fn duration_since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::ZERO)
}
