use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified_unix: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: Uuid,
    pub entries: Vec<ManifestEntry>,
    pub total_bytes: u64,
}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            id: Uuid::new_v4(),
            entries: Vec::new(),
            total_bytes: 0,
        }
    }
}
