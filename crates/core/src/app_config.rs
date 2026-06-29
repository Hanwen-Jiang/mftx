use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};

use mft_protocol::crypto::PasswordRecord;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

pub const DEFAULT_DIR_NAME: &str = "mftx";
pub const CONFIG_FILE_NAME: &str = "config.json";
pub const LOCATION_FILE_NAME: &str = "location.json";
pub const DEFAULT_TRANSFER_PORT: u16 = 48151;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "Uuid::new_v4")]
    pub device_id: Uuid,
    pub device_name: String,
    pub listen_addr: SocketAddr,
    pub password: PasswordRecord,
    pub base_dir: PathBuf,
    pub inbox_dir: PathBuf,
    pub share_dir: PathBuf,
    pub received_dir: PathBuf,
    /// Explicit discovery targets (host or host:port) probed in addition to
    /// UDP broadcast + ARP neighbors. Required to discover peers across overlay
    /// networks such as Tailscale, which carry no broadcast/multicast traffic.
    /// Empty = today's broadcast-only behavior (backward compatible).
    #[serde(default)]
    pub discovery_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppLocation {
    base_dir: PathBuf,
}

impl AppConfig {
    pub fn new(
        device_name: impl Into<String>,
        listen_addr: SocketAddr,
        password: PasswordRecord,
        base_dir: impl Into<PathBuf>,
    ) -> Self {
        let base_dir = base_dir.into();
        Self {
            device_id: Uuid::new_v4(),
            device_name: device_name.into(),
            listen_addr,
            password,
            inbox_dir: base_dir.join("inbox"),
            share_dir: base_dir.join("share"),
            received_dir: base_dir.join("received"),
            base_dir,
            discovery_targets: Vec::new(),
        }
    }

    pub fn default_listen_addr() -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_TRANSFER_PORT,
        ))
    }

    pub fn default_base_dir() -> anyhow::Result<PathBuf> {
        Ok(dirs::home_dir()
            .unwrap_or(std::env::current_dir()?)
            .join(DEFAULT_DIR_NAME))
    }

    pub fn location_path() -> anyhow::Result<PathBuf> {
        Ok(dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("cannot locate user config directory"))?
            .join(DEFAULT_DIR_NAME)
            .join(LOCATION_FILE_NAME))
    }

    pub async fn resolve_base_dir(input: Option<PathBuf>) -> anyhow::Result<PathBuf> {
        if let Some(base_dir) = input {
            return Ok(base_dir);
        }
        if let Some(base_dir) = std::env::var_os("MFTX_HOME").map(PathBuf::from) {
            return Ok(base_dir);
        }
        if let Ok(location_path) = Self::location_path() {
            if let Ok(data) = fs::read(&location_path).await {
                let location: AppLocation = serde_json::from_slice(&data)?;
                return Ok(location.base_dir);
            }
        }
        Self::default_base_dir()
    }

    pub fn config_path_for_base(base_dir: impl AsRef<Path>) -> PathBuf {
        base_dir.as_ref().join(CONFIG_FILE_NAME)
    }

    pub fn config_path(&self) -> PathBuf {
        Self::config_path_for_base(&self.base_dir)
    }

    pub fn default_share_paths(&self) -> Vec<PathBuf> {
        vec![self.share_dir.clone()]
    }

    pub async fn ensure_dirs(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.base_dir).await?;
        fs::create_dir_all(&self.inbox_dir).await?;
        fs::create_dir_all(&self.share_dir).await?;
        fs::create_dir_all(&self.received_dir).await?;
        Ok(())
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        self.ensure_dirs().await?;
        let config_path = self.config_path();
        let data = serde_json::to_vec_pretty(self)?;
        fs::write(&config_path, data).await?;
        set_owner_only_permissions(&config_path)?;
        Ok(())
    }

    pub async fn save_location(&self) -> anyhow::Result<()> {
        let location_path = Self::location_path()?;
        if let Some(parent) = location_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_vec_pretty(&AppLocation {
            base_dir: self.base_dir.clone(),
        })?;
        fs::write(&location_path, data).await?;
        set_owner_only_permissions(&location_path)?;
        Ok(())
    }

    pub async fn load_from_base(base_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let base_dir = base_dir.as_ref();
        let config_path = Self::config_path_for_base(base_dir);
        let data = fs::read(&config_path)
            .await
            .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", config_path.display()))?;
        let config: Self = serde_json::from_slice(&data)?;
        let should_migrate = !data_contains_field(&data, "device_id") && !data_contains_field(&data, "deviceId");
        config.ensure_dirs().await?;
        if should_migrate {
            config.save().await?;
        }
        Ok(config)
    }

    pub async fn load(input: Option<PathBuf>) -> anyhow::Result<Self> {
        let base_dir = Self::resolve_base_dir(input).await?;
        Self::load_from_base(base_dir).await
    }
}

fn data_contains_field(data: &[u8], field: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(data)
        .ok()
        .and_then(|value| value.as_object().map(|object| object.contains_key(field)))
        .unwrap_or(false)
}

fn set_owner_only_permissions(_path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(_path)?.permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(_path, permissions)?;
    }
    Ok(())
}
