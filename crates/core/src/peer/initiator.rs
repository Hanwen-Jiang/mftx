use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::transfer::TransferReport;

pub async fn push_paths(
    addr: SocketAddr,
    password: &str,
    paths: &[PathBuf],
) -> anyhow::Result<TransferReport> {
    crate::transfer::upload_paths(addr, password, paths).await
}

pub async fn pull_all(
    addr: SocketAddr,
    password: &str,
    out_dir: impl AsRef<Path>,
) -> anyhow::Result<TransferReport> {
    crate::transfer::download_all(addr, password, out_dir).await
}
