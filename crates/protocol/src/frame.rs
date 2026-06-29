use serde::{Deserialize, Serialize};

use crate::manifest::{Manifest, ManifestEntry};
use uuid::Uuid;

pub const PROTOCOL_MAGIC: &str = "MFT1";
pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Hello {
        device_id: Option<Uuid>,
        device_name: String,
        version: u16,
        client_nonce: [u8; 32],
    },
    HelloAck {
        device_id: Option<Uuid>,
        device_name: String,
        version: u16,
        server_nonce: [u8; 32],
        password_salt_hex: String,
    },
    Auth {
        device_id: Option<Uuid>,
        device_name: String,
    },
    AuthOk {
        manifest: Manifest,
    },
    Error {
        code: String,
        message: String,
    },
    Manifest(Manifest),
    TransferOffer {
        offer_id: Uuid,
        device_id: Uuid,
        device_name: String,
        manifest: Manifest,
        files: usize,
        bytes: u64,
    },
    TransferDecision {
        offer_id: Uuid,
        accepted: bool,
        message: Option<String>,
    },
    GetFile {
        path: String,
        offset: u64,
    },
    PutFileStart {
        entry: ManifestEntry,
    },
    FileChunk {
        path: String,
        offset: u64,
        data: Vec<u8>,
        last: bool,
    },
    FileDone {
        path: String,
        size: u64,
        blake3_hex: String,
    },
    Ack {
        path: String,
    },
    Done,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame encode/decode failed: {0}")]
    Codec(#[from] Box<bincode::ErrorKind>),
    #[error("frame is too large: {0} bytes")]
    TooLarge(usize),
}

pub fn encode_plain(frame: &Frame) -> Result<Vec<u8>, FrameError> {
    bincode::serialize(frame).map_err(FrameError::from)
}

pub fn decode_plain(bytes: &[u8]) -> Result<Frame, FrameError> {
    bincode::deserialize(bytes).map_err(FrameError::from)
}
