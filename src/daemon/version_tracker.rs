use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionState {
    pub last_seen_version: String,
    pub last_seen_date: DateTime<Utc>,
    pub show_changelog: bool,
}

impl VersionState {
    fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".mc-manager")
            .join("version_state.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(state) = serde_json::from_str(&content) {
                    return state;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&path, serde_json::to_string_pretty(self).unwrap()).ok();
    }

    pub fn default() -> Self {
        Self {
            last_seen_version: "0.0.0".to_string(),
            last_seen_date: Utc::now(),
            show_changelog: true,
        }
    }

    pub fn is_new_version(&self, current_version: &str) -> bool {
        self.last_seen_version != current_version
    }

    pub fn mark_version_seen(&mut self, current_version: &str) {
        self.last_seen_version = current_version.to_string();
        self.last_seen_date = Utc::now();
        self.save();
    }
}

pub fn fetch_changelog_for_version(version: &str) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "https://api.github.com/repos/FerNandoGD24/Minecraft-Server-Manager-Rust/releases/tags/v{}",
        version
    );

    #[derive(serde::Deserialize)]
    struct GitHubRelease {
        body: String,
        html_url: String,
    }

    let release: GitHubRelease = reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", "mc-manager-updater")
        .send()?
        .json()?;

    Ok(release.body)
}