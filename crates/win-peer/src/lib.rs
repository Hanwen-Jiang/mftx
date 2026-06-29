use std::net::SocketAddr;
use std::path::PathBuf;

use mft_core::discovery::DiscoveryBeacon;

pub fn choose_connect_addr(
    manual: Option<SocketAddr>,
    peers: &[DiscoveryBeacon],
) -> anyhow::Result<SocketAddr> {
    if let Some(addr) = manual {
        return Ok(addr);
    }

    let peer = peers
        .iter()
        .find(|peer| {
            peer.capabilities
                .iter()
                .any(|capability| capability == "send")
        })
        .or_else(|| peers.first())
        .ok_or_else(|| anyhow::anyhow!("no MFT peer discovered; pass --connect <ip:port>"))?;

    peer.observed_addr
        .ok_or_else(|| anyhow::anyhow!("discovered peer has no observed address"))
}

pub fn format_peer(peer: &DiscoveryBeacon) -> String {
    let addr = peer
        .observed_addr
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| format!("unknown:{}", peer.port));
    format!(
        "{}  {}  session={}  capabilities={}",
        peer.device_name,
        addr,
        peer.session_id,
        peer.capabilities.join(",")
    )
}

pub fn validate_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    if paths.is_empty() {
        anyhow::bail!("at least one path is required");
    }
    for path in paths {
        if !path.exists() {
            anyhow::bail!("path does not exist: {}", path.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    use mft_core::discovery::DiscoveryBeacon;
    use uuid::Uuid;

    use super::{choose_connect_addr, format_peer};

    #[test]
    fn manual_connect_address_takes_priority_over_discovery() {
        let manual = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 5), 48151));

        assert_eq!(choose_connect_addr(Some(manual), &[]).unwrap(), manual);
    }

    #[test]
    fn discovery_prefers_peer_that_can_send_files() {
        let receive_only_addr =
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 48151));
        let sender_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 21), 48151));
        let mut receive_only = DiscoveryBeacon::new(
            Uuid::new_v4(),
            "receive-only".to_string(),
            receive_only_addr,
            vec!["receive".to_string()],
        );
        receive_only.observed_addr = Some(receive_only_addr);
        let mut sender = DiscoveryBeacon::new(
            Uuid::new_v4(),
            "sender".to_string(),
            sender_addr,
            vec!["send".to_string()],
        );
        sender.observed_addr = Some(sender_addr);

        assert_eq!(
            choose_connect_addr(None, &[receive_only, sender]).unwrap(),
            sender_addr
        );
    }

    #[test]
    fn empty_discovery_result_has_actionable_error() {
        let error = choose_connect_addr(None, &[]).unwrap_err().to_string();

        assert!(error.contains("--connect <ip:port>"));
    }

    #[test]
    fn peer_output_uses_observed_address_and_capabilities() {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 30), 48151));
        let mut peer = DiscoveryBeacon::new(
            Uuid::new_v4(),
            "MacBook".to_string(),
            addr,
            vec!["send".to_string(), "receive".to_string()],
        );
        peer.observed_addr = Some(addr);

        let output = format_peer(&peer);

        assert!(output.contains("MacBook"));
        assert!(output.contains("192.168.1.30:48151"));
        assert!(output.contains("send,receive"));
        assert!(!output.contains("secret.txt"));
    }
}
