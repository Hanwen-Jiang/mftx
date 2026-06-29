use std::net::SocketAddr;
use std::time::Duration;

use uuid::Uuid;

use super::table::PeerRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Push,
    Pull,
}

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    PeerOnline {
        name: String,
        addr: SocketAddr,
    },
    PeerDiscovered {
        peer: PeerRecord,
    },
    PeerExpired {
        session_id: Uuid,
    },
    TransferStarted {
        id: Uuid,
        direction: Direction,
        peer: String,
        files: usize,
        bytes: u64,
    },
    FileStarted {
        id: Uuid,
        path: String,
        size: u64,
    },
    FileProgress {
        id: Uuid,
        path: String,
        written: u64,
        total: u64,
        bytes_per_sec: f64,
    },
    FileFinished {
        id: Uuid,
        path: String,
        blake3_hex: String,
    },
    TransferFinished {
        id: Uuid,
        files: usize,
        bytes: u64,
        elapsed: Duration,
    },
    TransferFailed {
        id: Option<Uuid>,
        code: String,
        message: String,
    },
}
