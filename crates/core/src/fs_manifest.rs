use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use mft_protocol::manifest::{EntryKind, Manifest, ManifestEntry};
use mft_protocol::path::clean_relative_path;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ManifestBundle {
    pub manifest: Manifest,
    pub files: HashMap<String, PathBuf>,
}

pub async fn build_manifest(paths: &[PathBuf]) -> anyhow::Result<Manifest> {
    Ok(build_manifest_bundle(paths).await?.manifest)
}

pub async fn build_manifest_bundle(paths: &[PathBuf]) -> anyhow::Result<ManifestBundle> {
    let mut entries = Vec::new();
    let mut files = HashMap::new();
    let mut total_bytes = 0_u64;

    for input in paths {
        let metadata = tokio::fs::metadata(input).await?;
        let base = input
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow::anyhow!("path has no valid file name: {}", input.display()))?
            .to_string();

        if metadata.is_file() {
            let path = clean_relative_path(&base)?;
            total_bytes += metadata.len();
            files.insert(path.clone(), input.clone());
            entries.push(entry_for(&path, EntryKind::File, &metadata));
            continue;
        }

        if metadata.is_dir() {
            for item in WalkDir::new(input).follow_links(false).sort_by_file_name() {
                let item = item?;
                let item_path = item.path();
                let item_metadata = item.metadata()?;
                if item.file_type().is_symlink() {
                    continue;
                }
                let relative = relative_name(input, item_path, &base)?;
                if relative.is_empty() {
                    let clean = clean_relative_path(&base)?;
                    entries.push(entry_for(&clean, EntryKind::Directory, &item_metadata));
                    continue;
                }
                let clean = clean_relative_path(format!("{base}/{relative}"))?;
                if item_metadata.is_dir() {
                    entries.push(entry_for(&clean, EntryKind::Directory, &item_metadata));
                } else if item_metadata.is_file() {
                    total_bytes += item_metadata.len();
                    files.insert(clean.clone(), item_path.to_path_buf());
                    entries.push(entry_for(&clean, EntryKind::File, &item_metadata));
                }
            }
        }
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries.dedup_by(|left, right| left.path == right.path);

    Ok(ManifestBundle {
        manifest: Manifest {
            id: Uuid::new_v4(),
            entries,
            total_bytes,
        },
        files,
    })
}

fn relative_name(root: &Path, path: &Path, base: &str) -> anyhow::Result<String> {
    if path == root {
        return Ok(String::new());
    }
    let relative = path.strip_prefix(root)?;
    let text = relative
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-utf8 path below {base}"))?;
    Ok(text.replace('\\', "/"))
}

fn entry_for(path: &str, kind: EntryKind, metadata: &std::fs::Metadata) -> ManifestEntry {
    ManifestEntry {
        path: path.to_string(),
        kind,
        size: if kind == EntryKind::File {
            metadata.len()
        } else {
            0
        },
        modified_unix: metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64),
    }
}
