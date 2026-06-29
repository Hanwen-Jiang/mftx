#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferLimits {
    pub chunk_bytes: usize,
    pub max_frame_bytes: usize,
    pub max_concurrent_inbound: usize,
    pub max_manifest_entries: usize,
    pub max_path_bytes: usize,
}

impl Default for TransferLimits {
    fn default() -> Self {
        Self {
            chunk_bytes: 1024 * 1024,
            max_frame_bytes: 8 * 1024 * 1024,
            max_concurrent_inbound: 4,
            max_manifest_entries: 200_000,
            max_path_bytes: 4096,
        }
    }
}
