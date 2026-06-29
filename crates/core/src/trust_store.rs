use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

pub const TRUSTED_DEVICES_FILE_NAME: &str = "trusted-devices.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub device_id: Uuid,
    pub display_name: String,
    pub first_trusted_at_ms: i64,
    pub last_seen_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedDevices {
    pub devices: Vec<TrustedDevice>,
}

#[derive(Debug, Clone)]
pub struct TrustedDeviceStore {
    path: PathBuf,
}

impl TrustedDeviceStore {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            path: base_dir.as_ref().join(TRUSTED_DEVICES_FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn load(&self) -> anyhow::Result<TrustedDevices> {
        match fs::read(&self.path).await {
            Ok(data) => Ok(serde_json::from_slice(&data)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(TrustedDevices::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn save(&self, trusted: &TrustedDevices) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_vec_pretty(trusted)?;
        fs::write(&self.path, data).await?;
        Ok(())
    }

    pub async fn list(&self) -> anyhow::Result<Vec<TrustedDevice>> {
        Ok(self.load().await?.devices)
    }

    pub async fn contains(&self, device_id: Uuid) -> anyhow::Result<bool> {
        Ok(self
            .load()
            .await?
            .devices
            .iter()
            .any(|device| device.device_id == device_id))
    }

    pub async fn trust(
        &self,
        device_id: Uuid,
        display_name: impl Into<String>,
        trusted_at_ms: i64,
    ) -> anyhow::Result<TrustedDevice> {
        let display_name = display_name.into();
        let mut trusted = self.load().await?;
        if let Some(device) = trusted
            .devices
            .iter_mut()
            .find(|device| device.device_id == device_id)
        {
            device.display_name = display_name;
            device.last_seen_at_ms = Some(trusted_at_ms);
            let device = device.clone();
            self.save(&trusted).await?;
            return Ok(device);
        }

        let device = TrustedDevice {
            device_id,
            display_name,
            first_trusted_at_ms: trusted_at_ms,
            last_seen_at_ms: Some(trusted_at_ms),
        };
        trusted.devices.push(device.clone());
        trusted.devices.sort_by(|a, b| {
            a.display_name
                .cmp(&b.display_name)
                .then_with(|| a.device_id.cmp(&b.device_id))
        });
        self.save(&trusted).await?;
        Ok(device)
    }

    pub async fn untrust(&self, device_id: Uuid) -> anyhow::Result<bool> {
        let mut trusted = self.load().await?;
        let before = trusted.devices.len();
        trusted.devices.retain(|device| device.device_id != device_id);
        let removed = trusted.devices.len() != before;
        if removed {
            self.save(&trusted).await?;
        }
        Ok(removed)
    }
}
