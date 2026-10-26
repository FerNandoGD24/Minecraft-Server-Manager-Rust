use crate::daemon::types::{DaemonConfig, UpdateState, UpdateMarker, IpcRequest, IpcResponse, UpdateStatus};
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use semver::Version;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::process::Command;

const GITHUB_REPO: &str = "FerNandoGD24/Minecraft-Server-Manager-Rust";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: String,
    html_url: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

pub struct AutoUpdater {
    config: DaemonConfig,
    client: Client,
    current_version: Version,
    running: Arc<AtomicBool>,
}

impl AutoUpdater {
    pub fn new(config: DaemonConfig) -> Self {
        let current_version = Version::parse(CURRENT_VERSION).unwrap_or_else(|_| Version::new(0, 0, 0));
        Self {
            config,
            client: Client::new(),
            current_version,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn check_and_prepare(&mut self) -> Result<()> {
        let release = self.fetch_latest_release()?;
        let latest_version = Version::parse(release.tag_name.trim_start_matches('v'))
            .unwrap_or_else(|_| Version::new(0, 0, 0));

        if latest_version > self.current_version {
            if let Some(asset) = self.find_asset(&release) {
                self.download_and_verify(&asset.browser_download_url, &asset.name)?;
                let state = UpdateState {
                    pending: true,
                    version: release.tag_name.trim_start_matches('v').to_string(),
                    method: "auto".to_string(),
                    temp_path: self.temp_path(&asset.name).to_string_lossy().to_string(),
                    downloaded_at: chrono::Utc::now().to_rfc3339(),
                };
                self.save_state(&state)?;
            }
        }
        Ok(())
    }

    pub fn apply_on_shutdown(&self) -> Result<()> {
        if let Some(state) = self.load_state()? {
            if state.pending {
                self.apply_update(&state)?;
                let marker = UpdateMarker {
                    updated: true,
                    version: state.version,
                    method: state.method,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                self.save_marker(&marker)?;
                self.clear_state()?;
            }
        }
        Ok(())
    }

    pub fn apply_now_via_ipc(&self) -> Result<()> {
        if let Some(state) = self.load_state()? {
            if state.pending {
                self.apply_update(&state)?;
                let marker = UpdateMarker {
                    updated: true,
                    version: state.version,
                    method: "manual".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                self.save_marker(&marker)?;
                self.clear_state()?;
            }
        }
        Ok(())
    }

    pub fn check_for_updates_interactive(&self) -> Result<UpdateStatus> {
        let release = self.fetch_latest_release()?;
        let latest_version = Version::parse(release.tag_name.trim_start_matches('v'))
            .unwrap_or_else(|_| Version::new(0, 0, 0));

        if latest_version > self.current_version {
            let asset = self.find_asset(&release);
            if let Some(asset) = asset {
                let downloaded = self.is_downloaded(&asset.name);
                return Ok(UpdateStatus::UpdateAvailable {
                    version: release.tag_name.trim_start_matches('v').to_string(),
                    download_url: asset.browser_download_url.clone(),
                    downloaded,
                });
            }
        }
        Ok(UpdateStatus::UpToDate)
    }

    pub fn download_and_prepare(&self, download_url: &str) -> Result<()> {
        let temp_path = self.temp_path("mc-manager.new");
        self.download_and_verify(download_url, "mc-manager.new")?;
        let state = UpdateState {
            pending: true,
            version: "latest".to_string(),
            method: "manual".to_string(),
            temp_path: temp_path.to_string_lossy().to_string(),
            downloaded_at: chrono::Utc::now().to_rfc3339(),
        };
        self.save_state(&state)
    }

    fn fetch_latest_release(&self) -> Result<GitHubRelease> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
        let response = self.client
            .get(&url)
            .header("User-Agent", "mc-manager-updater")
            .send()?
            .json()?;
        Ok(response)
    }

    fn find_asset(&self, release: &GitHubRelease) -> Option<ReleaseAsset> {
        let arch = std::env::consts::ARCH;
        let asset_name = match std::env::consts::ARCH {
            "x86_64" => "mc-manager-linux-x86_64",
            "aarch64" => "mc-manager-linux-aarch64",
            _ => return None,
        };
        release.assets.iter().find(|a| a.name == asset_name).cloned()
    }

    fn download_and_verify(&self, url: &str, name: &str) -> Result<()> {
        let temp_path = self.temp_path(name);
        let bytes = self.client.get(url).send()?.bytes()?;

        if bytes.len() < 100_000 {
            return Err(anyhow!("Binario demasiado pequeno: {} bytes", bytes.len()));
        }

        if let Some(expected) = self.find_asset(&self.fetch_latest_release()?) {
            if let Some(expected_sha) = expected.digest {
                let expected_sha = expected_sha.trim_start_matches("sha256:");
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
            let actual = hex::encode(hasher.finalize());
                if actual != expected_sha {
                    return Err(anyhow!("Checksum invalido: esperado {}, actual {}", expected_sha, actual));
                }
            }
        }

        std::fs::write(&temp_path, &bytes)?;
        Ok(())
    }

    fn temp_path(&self, name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn is_downloaded(&self, name: &str) -> bool {
        self.temp_path(name).exists()
    }

    fn save_state(&self, state: &UpdateState) -> Result<()> {
        let path = self.state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_string_pretty(state)?)?;
        Ok(())
    }

    fn load_state(&self) -> Result<Option<UpdateState>> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&content)?))
    }

    fn clear_state(&self) -> Result<()> {
        let path = self.state_path();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn state_path(&self) -> PathBuf {
        std::env::temp_dir().join("mc-manager-update.json")
    }

    fn save_marker(&self, marker: &UpdateMarker) -> Result<()> {
        let path = std::env::temp_dir().join("mc-manager-updated.json");
        fs::write(&path, serde_json::to_string_pretty(marker)?)?;
        Ok(())
    }

    fn apply_update(&self, state: &UpdateState) -> Result<()> {
        let temp_path = PathBuf::from(&state.temp_path);
        let target = std::env::current_exe()?;
        let backup = target.with_extension("old");

        std::fs::rename(&target, &backup)?;
        std::fs::rename(&temp_path, &target)?;
        
        let mut perms = fs::metadata(&target)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target, perms)?;

        Ok(())
    }
}

