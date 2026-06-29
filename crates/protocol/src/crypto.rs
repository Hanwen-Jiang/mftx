use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::frame::{decode_plain, encode_plain, Frame, FrameError};

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("argon2 key derivation failed")]
    Argon,
    #[error("frame error: {0}")]
    Frame(#[from] FrameError),
    #[error("encryption or authentication failed")]
    Aead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordRecord {
    pub salt_hex: String,
    pub key_hex: String,
}

impl PasswordRecord {
    pub fn create(password: &str) -> Result<Self, CryptoError> {
        let mut salt = [0_u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        let key = password_key(password, &salt)?;
        Ok(Self {
            salt_hex: hex::encode(salt),
            key_hex: hex::encode(key),
        })
    }

    pub fn verify(&self, password: &str) -> Result<bool, CryptoError> {
        let salt = hex::decode(&self.salt_hex)?;
        let expected = hex::decode(&self.key_hex)?;
        let actual = password_key(password, &salt)?;
        Ok(constant_time_eq(&expected, &actual))
    }

    pub fn key_bytes(&self) -> Result<[u8; 32], CryptoError> {
        let bytes = hex::decode(&self.key_hex)?;
        let mut out = [0_u8; 32];
        out.copy_from_slice(bytes.get(..32).ok_or(CryptoError::Argon)?);
        Ok(out)
    }
}

pub fn derive_session_key(
    password: &str,
    record: &PasswordRecord,
    client_nonce: [u8; 32],
    server_nonce: [u8; 32],
) -> Result<[u8; 32], CryptoError> {
    derive_session_key_from_password_and_salt(
        password,
        &record.salt_hex,
        client_nonce,
        server_nonce,
    )
}

pub fn derive_session_key_from_password_and_salt(
    password: &str,
    salt_hex: &str,
    client_nonce: [u8; 32],
    server_nonce: [u8; 32],
) -> Result<[u8; 32], CryptoError> {
    let salt = hex::decode(salt_hex)?;
    let seed = password_key(password, &salt)?;
    Ok(derive_session_key_from_seed(
        &seed,
        client_nonce,
        server_nonce,
    ))
}

pub fn derive_session_key_from_record(
    record: &PasswordRecord,
    client_nonce: [u8; 32],
    server_nonce: [u8; 32],
) -> Result<[u8; 32], CryptoError> {
    let seed = record.key_bytes()?;
    Ok(derive_session_key_from_seed(
        &seed,
        client_nonce,
        server_nonce,
    ))
}

fn derive_session_key_from_seed(
    seed: &[u8; 32],
    client_nonce: [u8; 32],
    server_nonce: [u8; 32],
) -> [u8; 32] {
    let mut material = Vec::with_capacity(96);
    material.extend_from_slice(seed);
    material.extend_from_slice(&client_nonce);
    material.extend_from_slice(&server_nonce);
    blake3::derive_key("mft-v1-session-key", &material)
}

fn password_key(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let params = Params::new(19 * 1024, 2, 1, Some(32)).map_err(|_| CryptoError::Argon)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0_u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|_| CryptoError::Argon)?;
    Ok(out)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Which side of a session a [`SessionCipher`] represents.
///
/// Both peers derive the *same* session key, so the nonce space must be
/// domain-separated by direction; otherwise the initiator's frame N and the
/// responder's frame N would reuse the same `(key, nonce)` pair — catastrophic
/// for ChaCha20-Poly1305 (keystream reuse + Poly1305 forgery).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Initiator,
    Responder,
}

// Direction byte mixed into the AEAD nonce so the two halves of a session never
// collide. Initiator->responder traffic uses 0, responder->initiator uses 1.
const NONCE_DIR_INITIATOR_TO_RESPONDER: u8 = 0;
const NONCE_DIR_RESPONDER_TO_INITIATOR: u8 = 1;

#[derive(Clone)]
pub struct SessionCipher {
    cipher: ChaCha20Poly1305,
    tx_counter: u64,
    rx_counter: u64,
    tx_dir: u8,
    rx_dir: u8,
}

impl SessionCipher {
    pub fn new(key: [u8; 32], role: SessionRole) -> Self {
        let (tx_dir, rx_dir) = match role {
            SessionRole::Initiator => (
                NONCE_DIR_INITIATOR_TO_RESPONDER,
                NONCE_DIR_RESPONDER_TO_INITIATOR,
            ),
            SessionRole::Responder => (
                NONCE_DIR_RESPONDER_TO_INITIATOR,
                NONCE_DIR_INITIATOR_TO_RESPONDER,
            ),
        };
        Self {
            cipher: ChaCha20Poly1305::new(Key::from_slice(&key)),
            tx_counter: 0,
            rx_counter: 0,
            tx_dir,
            rx_dir,
        }
    }

    pub fn seal(&mut self, frame: &Frame) -> Result<Vec<u8>, CryptoError> {
        let plain = encode_plain(frame)?;
        let nonce = counter_nonce(self.tx_dir, self.tx_counter);
        self.tx_counter += 1;
        self.cipher
            .encrypt(Nonce::from_slice(&nonce), plain.as_slice())
            .map_err(|_| CryptoError::Aead)
    }

    pub fn open(&mut self, sealed: &[u8]) -> Result<Frame, CryptoError> {
        let nonce = counter_nonce(self.rx_dir, self.rx_counter);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(&nonce), sealed)
            .map_err(|_| CryptoError::Aead)?;
        self.rx_counter += 1;
        decode_plain(&plain).map_err(CryptoError::from)
    }
}

fn counter_nonce(direction: u8, counter: u64) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[0] = direction;
    nonce[4..].copy_from_slice(&counter.to_be_bytes());
    nonce
}
