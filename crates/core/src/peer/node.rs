use std::net::SocketAddr;
use std::sync::Arc;

use crate::discovery::DiscoveryBeacon;
use crate::transfer::TransferServer;
use uuid::Uuid;

use super::config::PeerConfig;
use super::discovery_service::DiscoveryService;
use super::table::PeerTable;

#[derive(Debug)]
pub struct PeerNode {
    addr: SocketAddr,
    config: Arc<PeerConfig>,
    peer_table: Arc<PeerTable>,
    session_id: Uuid,
    _server: TransferServer,
    _discovery: Option<DiscoveryService>,
}

impl PeerNode {
    pub async fn bind(config: PeerConfig) -> anyhow::Result<Self> {
        config.validate()?;
        let server = TransferServer::bind_with_handler(
            config.listen_addr,
            config.password.clone(),
            config.share_paths.clone(),
            config.inbox_dir.clone(),
            config.incoming_handler.clone(),
        )
        .await?;
        let addr = server.addr();
        let peer_table = Arc::new(PeerTable::default());
        let beacon = DiscoveryBeacon::new(
            config.device_id,
            config.device_name.clone(),
            addr,
            config.capabilities.to_wire_strings(),
        );
        let session_id = beacon.session_id;
        let discovery = if config.enable_discovery {
            Some(DiscoveryService::spawn(
                beacon,
                Arc::clone(&peer_table),
                config.announce_interval,
            ))
        } else {
            None
        };

        Ok(Self {
            addr,
            config: Arc::new(config),
            peer_table,
            session_id,
            _server: server,
            _discovery: discovery,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn config(&self) -> &PeerConfig {
        &self.config
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn peer_table(&self) -> Arc<PeerTable> {
        Arc::clone(&self.peer_table)
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        drop(self);
        Ok(())
    }
}
