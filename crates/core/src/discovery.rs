use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use uuid::Uuid;

pub const DISCOVERY_PORT: u16 = 48150;
const DISCOVERY_MAGIC: &str = "MFT_DISCOVERY_V1";
const MAX_ARP_DISCOVERY_TARGETS: usize = 256;

/// How long a parsed ARP snapshot stays valid before the next discovery call
/// re-runs `arp -a`. Spawning `arp.exe` on every sweep (and several times per
/// sweep) exhausted the desktop's commit/handle budget on Windows, crashing
/// arp.exe/conhost.exe with 0xC000012D. We now spawn it at most once per window
/// and share the parsed result across every per-sweep call site.
const ARP_CACHE_TTL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryMessageKind {
    Beacon,
    Probe,
}

fn default_message_kind() -> DiscoveryMessageKind {
    DiscoveryMessageKind::Beacon
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryBeacon {
    pub magic: String,
    #[serde(default = "default_message_kind")]
    pub kind: DiscoveryMessageKind,
    pub version: u16,
    #[serde(default = "Uuid::new_v4")]
    pub device_id: Uuid,
    pub device_name: String,
    pub port: u16,
    pub session_id: Uuid,
    pub capabilities: Vec<String>,
    #[serde(skip)]
    pub observed_addr: Option<SocketAddr>,
}

impl DiscoveryBeacon {
    pub fn new(
        device_id: Uuid,
        device_name: String,
        addr: SocketAddr,
        capabilities: Vec<String>,
    ) -> Self {
        Self {
            magic: DISCOVERY_MAGIC.to_string(),
            kind: DiscoveryMessageKind::Beacon,
            version: mft_protocol::frame::PROTOCOL_VERSION,
            device_id,
            device_name,
            port: addr.port(),
            session_id: Uuid::new_v4(),
            capabilities,
            observed_addr: None,
        }
    }

    pub fn probe(device_name: String) -> Self {
        Self {
            magic: DISCOVERY_MAGIC.to_string(),
            kind: DiscoveryMessageKind::Probe,
            version: mft_protocol::frame::PROTOCOL_VERSION,
            device_id: Uuid::new_v4(),
            device_name,
            port: 0,
            session_id: Uuid::new_v4(),
            capabilities: Vec::new(),
            observed_addr: None,
        }
    }

    pub fn is_probe(&self) -> bool {
        self.kind == DiscoveryMessageKind::Probe
    }

    pub fn to_wire(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_wire(input: &str) -> serde_json::Result<Self> {
        let beacon: Self = serde_json::from_str(input)?;
        if beacon.magic == DISCOVERY_MAGIC {
            Ok(beacon)
        } else {
            Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid discovery magic",
            )))
        }
    }
}

pub async fn broadcast_once(beacon: &DiscoveryBeacon) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;
    socket.set_broadcast(true)?;
    send_to_discovery_targets(&socket, beacon).await?;
    Ok(())
}

pub async fn discover_for(timeout: Duration) -> anyhow::Result<Vec<DiscoveryBeacon>> {
    discover_for_with_responder(timeout, None).await
}

pub async fn discover_for_with_responder(
    timeout: Duration,
    responder: Option<&DiscoveryBeacon>,
) -> anyhow::Result<Vec<DiscoveryBeacon>> {
    discover_for_streaming(timeout, responder, |_| {}).await
}

/// Like [`discover_for_with_responder`], but invokes `on_peer` the instant each
/// new (session-unique) beacon arrives — so callers can surface fast responders
/// immediately instead of waiting for the whole window to close. Slow/high-
/// latency devices simply stream in later; they never gate the fast ones. The
/// full deduplicated list is still returned when the window elapses.
pub async fn discover_for_streaming<F: FnMut(&DiscoveryBeacon)>(
    timeout: Duration,
    responder: Option<&DiscoveryBeacon>,
    mut on_peer: F,
) -> anyhow::Result<Vec<DiscoveryBeacon>> {
    // Bind DISCOVERY_PORT so this socket receives BOTH the remote's passive
    // broadcast beacons AND unicast probe-replies. (The Windows-specific dual-
    // socket contention that happens when a co-located peer also binds this port
    // is handled at a higher level — the desktop reads the running peer's own
    // PeerTable instead of opening a second socket — so we never rely on an
    // ephemeral port here, which would miss passive broadcasts.)
    let socket = reusable_discovery_socket(DISCOVERY_PORT)?;
    let local_ipv4s = local_discoverable_ipv4s().into_iter().collect::<HashSet<_>>();
    let probe = DiscoveryBeacon::probe(hostname_label());
    let _ = send_to_discovery_targets(&socket, &probe).await;
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut buf = vec![0_u8; 2048];
    let mut beacons = Vec::new();

    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                let (len, peer) = match result {
                    Ok(value) => value,
                    // On Windows, a prior probe sent to a UDP port with no listener
                    // bounces back as ICMP "port unreachable", which surfaces on the
                    // NEXT recv_from as WSAECONNRESET (os error 10054). It is NOT
                    // fatal — many probe targets (offline Tailscale peers, non-MFTX
                    // hosts) legitimately have no listener — so skip it and keep
                    // receiving instead of aborting the whole discovery.
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                if is_local_peer_ip(peer.ip(), &local_ipv4s) {
                    continue;
                }
                if let Ok(text) = std::str::from_utf8(&buf[..len]) {
                    if let Ok(mut beacon) = DiscoveryBeacon::from_wire(text) {
                        if beacon.is_probe() {
                            if let Some(response) = responder {
                                let _ = socket.send_to(response.to_wire()?.as_bytes(), peer).await;
                            }
                            continue;
                        }
                        beacon.observed_addr = Some(SocketAddr::new(peer.ip(), beacon.port));
                        if !beacons.iter().any(|seen: &DiscoveryBeacon| seen.session_id == beacon.session_id) {
                            on_peer(&beacon);
                            beacons.push(beacon);
                        }
                    }
                }
            }
            _ = &mut deadline => return Ok(beacons),
        }
    }
}

async fn send_to_discovery_targets(
    socket: &UdpSocket,
    message: &DiscoveryBeacon,
) -> anyhow::Result<()> {
    let bytes = message.to_wire()?;
    let mut last_error = None;
    for target in discovery_targets() {
        if let Err(error) = socket.send_to(bytes.as_bytes(), target).await {
            last_error = Some(error);
        }
    }
    if let Some(error) = last_error {
        anyhow::bail!(error);
    }
    Ok(())
}

fn discovery_targets() -> Vec<SocketAddr> {
    let mut seen = HashSet::new();
    let mut targets = Vec::with_capacity(1 + MAX_ARP_DISCOVERY_TARGETS);

    let mut push = |target: SocketAddr, targets: &mut Vec<SocketAddr>| {
        if seen.insert(target) {
            targets.push(target);
        }
    };

    push(
        SocketAddr::from(([255, 255, 255, 255], DISCOVERY_PORT)),
        &mut targets,
    );

    // Directed broadcast on EVERY local interface, not just the primary route's —
    // covers multi-homed machines with several physical LAN ports / adapters.
    for ip in interface_broadcast_ips() {
        push(SocketAddr::from((ip, DISCOVERY_PORT)), &mut targets);
    }

    // Explicitly configured targets (any host/IP the user "connected" to — e.g. a
    // peer's overlay address). This is network-agnostic: it reaches peers that
    // broadcast + ARP cannot (different subnets, overlays with no broadcast),
    // without the app depending on or shelling out to any specific VPN/mesh tool.
    // Unicast probing these reaches peers that broadcast + ARP cannot — overlay
    // networks like Tailscale carry no broadcast/multicast and never appear in
    // the ARP table.
    for target in configured_discovery_targets() {
        push(target, &mut targets);
    }

    for ip in arp_candidate_ips()
        .into_iter()
        .take(MAX_ARP_DISCOVERY_TARGETS)
    {
        push(SocketAddr::from((ip, DISCOVERY_PORT)), &mut targets);
    }
    targets
}

/// Directed broadcast address of every non-loopback IPv4 interface. Sending the
/// probe to each one reaches all directly-attached LAN segments, not just the
/// interface that happens to own the default route.
fn interface_broadcast_ips() -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    let mut ips = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            if let if_addrs::IfAddr::V4(v4) = interface.addr {
                if v4.ip.is_loopback() {
                    continue;
                }
                if let Some(broadcast) = v4.broadcast {
                    if !broadcast.is_unspecified() && seen.insert(broadcast) {
                        ips.push(broadcast);
                    }
                }
            }
        }
    }
    ips
}


static CONFIGURED_TARGETS: OnceLock<Mutex<Vec<SocketAddr>>> = OnceLock::new();

/// Install the process-wide set of explicit discovery targets. Replaces any
/// previous set. An empty slice restores broadcast + ARP-only behavior, so this
/// is fully backward compatible. Call it whenever the app config changes.
pub fn set_discovery_targets(targets: Vec<SocketAddr>) {
    let cell = CONFIGURED_TARGETS.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = targets;
}

fn configured_discovery_targets() -> Vec<SocketAddr> {
    CONFIGURED_TARGETS
        .get()
        .map(|cell| {
            cell.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        })
        .unwrap_or_default()
}

/// Resolve user-entered discovery-target strings into socket addresses. Each
/// entry may be an `IP`, `IP:port`, `host`, or `host:port`; a missing port
/// defaults to [`DISCOVERY_PORT`]. Hostnames are resolved via the system
/// resolver. Unparseable/unresolvable entries are skipped rather than failing
/// the whole set. Result is de-duplicated, order-preserving.
pub fn parse_discovery_targets<I, S>(entries: I) -> Vec<SocketAddr>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for entry in entries {
        let trimmed = entry.as_ref().trim();
        if trimmed.is_empty() {
            continue;
        }
        for addr in resolve_discovery_target(trimmed) {
            if seen.insert(addr) {
                targets.push(addr);
            }
        }
    }
    targets
}

fn resolve_discovery_target(entry: &str) -> Vec<SocketAddr> {
    // Bare IPv4/IPv6 literal without a port → attach the discovery port.
    if let Ok(ip) = entry.parse::<IpAddr>() {
        return vec![SocketAddr::new(ip, DISCOVERY_PORT)];
    }
    // `host:port` / `ip:port` (or a resolvable host) → use as-is.
    if let Ok(addrs) = entry.to_socket_addrs() {
        return addrs.collect();
    }
    // Bare hostname without a port → resolve against the discovery port.
    if let Ok(addrs) = (entry, DISCOVERY_PORT).to_socket_addrs() {
        return addrs.collect();
    }
    Vec::new()
}

fn arp_candidate_ips() -> Vec<Ipv4Addr> {
    arp_snapshot().candidate_ips
}

pub fn local_discoverable_ipv4s() -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    let mut ips = Vec::new();

    for ip in arp_snapshot().local_ipv4s {
        if seen.insert(ip) {
            ips.push(ip);
        }
    }

    if let Some(ip) = primary_outbound_ipv4() {
        if seen.insert(ip) {
            ips.push(ip);
        }
    }

    ips
}

/// Parsed view of the system ARP table: neighbor IPs we can unicast-probe and
/// the local interface IPs we use to filter out our own beacons. Cloned out of
/// the cache so the mutex is never held while the rest of discovery runs.
#[derive(Clone, Default)]
struct ArpSnapshot {
    candidate_ips: Vec<Ipv4Addr>,
    local_ipv4s: Vec<Ipv4Addr>,
}

struct CachedArp {
    snapshot: ArpSnapshot,
    fetched_at: Instant,
}

/// Return a recent ARP snapshot, re-running `arp -a` only when the cache is
/// older than [`ARP_CACHE_TTL`]. Process-wide so the multiple call sites in a
/// single discovery sweep share one spawn instead of each launching arp.exe.
fn arp_snapshot() -> ArpSnapshot {
    static CACHE: OnceLock<Mutex<Option<CachedArp>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));

    // Poisoned mutex (a prior panic) shouldn't disable discovery — recover it.
    let mut guard = cache.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(cached) = guard.as_ref() {
        if cached.fetched_at.elapsed() < ARP_CACHE_TTL {
            return cached.snapshot.clone();
        }
    }

    let snapshot = fetch_arp_snapshot();
    *guard = Some(CachedArp {
        snapshot: snapshot.clone(),
        fetched_at: Instant::now(),
    });
    snapshot
}

/// Run `arp -a` once and parse both neighbor and local-interface IPs from the
/// single output. A spawn failure (arp.exe unavailable / resource pressure) is
/// swallowed: discovery still works over UDP broadcast and configured targets.
fn fetch_arp_snapshot() -> ArpSnapshot {
    let mut candidate_ips = Vec::new();
    let mut local_ipv4s = Vec::new();

    if let Ok(output) = command_output("arp", &["-a"]) {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            candidate_ips = parse_arp_candidate_ips(&text);
            local_ipv4s = parse_local_interface_ipv4s(&text);
        }
    }

    #[cfg(unix)]
    if candidate_ips.is_empty() {
        if let Ok(output) = command_output("ip", &["neigh", "show"]) {
            if output.status.success() {
                candidate_ips = parse_arp_candidate_ips(&String::from_utf8_lossy(&output.stdout));
            }
        }
    }

    ArpSnapshot {
        candidate_ips,
        local_ipv4s,
    }
}

pub fn parse_arp_candidate_ips(output: &str) -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    let mut ips = Vec::new();

    for line in output.lines() {
        for ip in parenthesized_ipv4s(line) {
            if is_discoverable_ipv4(ip) && seen.insert(ip) {
                ips.push(ip);
            }
        }

        let trimmed = line.trim_start();
        if is_interface_line(trimmed) {
            continue;
        }
        if !trimmed
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let Some(token) = trimmed.split_whitespace().next() else {
            continue;
        };
        if let Ok(ip) = token.parse::<Ipv4Addr>() {
            if is_discoverable_ipv4(ip) && seen.insert(ip) {
                ips.push(ip);
            }
        }
    }

    ips
}

pub fn parse_local_interface_ipv4s(output: &str) -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    let mut ips = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = interface_line_rest(trimmed) else {
            continue;
        };

        for token in rest.split_whitespace() {
            if let Some(ip) = parse_ipv4_token(token) {
                if is_discoverable_ipv4(ip) && seen.insert(ip) {
                    ips.push(ip);
                }
                break;
            }
        }
    }

    ips
}

fn is_interface_line(line: &str) -> bool {
    interface_line_rest(line).is_some()
}

fn interface_line_rest(line: &str) -> Option<&str> {
    line.strip_prefix("Interface:")
        .or_else(|| line.strip_prefix("接口:"))
}

fn parenthesized_ipv4s(line: &str) -> Vec<Ipv4Addr> {
    let mut ips = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('(') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find(')') else {
            break;
        };
        if let Ok(ip) = after_start[..end].parse::<Ipv4Addr>() {
            ips.push(ip);
        }
        rest = &after_start[end + 1..];
    }
    ips
}

fn parse_ipv4_token(token: &str) -> Option<Ipv4Addr> {
    token
        .trim_matches(|ch: char| ch != '.' && !ch.is_ascii_digit())
        .parse()
        .ok()
}

fn is_local_peer_ip(ip: IpAddr, local_ipv4s: &HashSet<Ipv4Addr>) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || local_ipv4s.contains(&ip),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}

fn is_discoverable_ipv4(ip: Ipv4Addr) -> bool {
    !ip.is_broadcast() && !ip.is_loopback() && !ip.is_multicast() && !ip.is_unspecified()
}

fn primary_outbound_ipv4() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if is_discoverable_ipv4(ip) => Some(ip),
        _ => None,
    }
}

fn command_output(program: &str, args: &[&str]) -> std::io::Result<Output> {
    let mut command = Command::new(program);
    command.args(args);
    suppress_command_window(&mut command);
    command.output()
}

#[cfg(windows)]
fn suppress_command_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn suppress_command_window(_command: &mut Command) {}

fn hostname_label() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "mft-discover".to_string())
}

fn reusable_discovery_socket(port: u16) -> anyhow::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, port).into())?;
    Ok(UdpSocket::from_std(socket.into())?)
}
