#[derive(Debug, thiserror::Error)]
pub enum PeerError {
    #[error("peer not found: {0}")]
    PeerNotFound(String),
    #[error("ambiguous peer: {0}")]
    AmbiguousPeer(String),
}
