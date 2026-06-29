use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::RwLock;
use std::time::Duration;

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRecord {
    pub device_id: Uuid,
    pub device_name: String,
    pub session_id: Uuid,
    pub addr: SocketAddr,
    pub capabilities: Vec<String>,
    pub version: u16,
    pub first_seen: Duration,
    pub last_seen: Duration,
}

#[derive(Debug, Default)]
pub struct PeerTable {
    peers: RwLock<HashMap<Uuid, PeerRecord>>,
}

impl PeerTable {
    pub fn upsert(&self, record: PeerRecord) {
        let mut peers = self.peers.write().expect("peer table lock poisoned");
        peers
            .entry(record.session_id)
            .and_modify(|existing| {
                existing.device_id = record.device_id;
                existing.device_name = record.device_name.clone();
                existing.addr = record.addr;
                existing.capabilities = record.capabilities.clone();
                existing.version = record.version;
                existing.last_seen = record.last_seen;
            })
            .or_insert(record);
    }

    pub fn list(&self) -> Vec<PeerRecord> {
        let mut peers: Vec<_> = self
            .peers
            .read()
            .expect("peer table lock poisoned")
            .values()
            .cloned()
            .collect();
        peers.sort_by(|a, b| a.device_name.cmp(&b.device_name).then(a.addr.cmp(&b.addr)));
        peers
    }

    pub fn resolve(&self, name_or_id: &str) -> anyhow::Result<PeerRecord> {
        let peers = self.peers.read().expect("peer table lock poisoned");
        if let Ok(session_id) = Uuid::parse_str(name_or_id) {
            return peers
                .get(&session_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no peer with session id {session_id}"));
        }

        if let Ok(addr) = name_or_id.parse::<SocketAddr>() {
            return peers
                .values()
                .find(|peer| peer.addr == addr)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no peer at {addr}"));
        }

        let matches: Vec<_> = peers
            .values()
            .filter(|peer| peer.device_name == name_or_id)
            .cloned()
            .collect();
        match matches.len() {
            0 => anyhow::bail!("no peer named {name_or_id}; use --connect <ip:port>"),
            1 => Ok(matches[0].clone()),
            _ => anyhow::bail!("multiple peers named {name_or_id}; use session id or --connect"),
        }
    }

    pub fn prune_older_than(&self, cutoff: Duration) {
        self.peers
            .write()
            .expect("peer table lock poisoned")
            .retain(|_, peer| peer.last_seen >= cutoff);
    }
}
