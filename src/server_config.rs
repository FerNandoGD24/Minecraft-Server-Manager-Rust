use crate::config::config_path;
use crate::discovery::types::ServerService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server_id: String,
    pub java_path: String,
    pub xms: String,
    pub xmx: String,
    pub jvm_extra_args: String,
    pub server_port: u16,
    pub max_players: u32,
    pub online_mode: bool,
    pub motd: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_id: String::new(),
            java_path: "java".to_string(),
            xms: "2G".to_string(),
            xmx: "4G".to_string(),
            jvm_extra_args: "-XX:+UseG1GC -XX:+ParallelRefProcEnabled".to_string(),
            server_port: 25565,
            max_players: 20,
            online_mode: true,
            motd: "Mi Servidor Minecraft".to_string(),
        }
    }
}

impl ServerConfig {
    fn config_dir() -> PathBuf {
        config_path().parent().unwrap().join("servers")
    }

    fn config_path(server_id: &str) -> PathBuf {
        Self::config_dir().join(format!("{}.json", server_id))
    }

    pub fn load(server_id: &str) -> Self {
        let path = Self::config_path(server_id);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<ServerConfig>(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path(&self.server_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn delete(server_id: &str) -> anyhow::Result<()> {
        let path = Self::config_path(server_id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn generate_start_sh(&self, server_dir: &PathBuf) -> anyhow::Result<()> {
        let content = format!(
            r#"#!/bin/bash
# Auto-generado por mc-manager - NO EDITAR MANUALMENTE
# Cambia la configuracion desde la TUI (tecla 'e')

{} -Xms{} -Xmx{} {} -jar server.jar nogui
"#,
            self.java_path, self.xms, self.xmx, self.jvm_extra_args
        );
        fs::write(server_dir.join("start.sh"), content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(server_dir.join("start.sh"), fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }

    pub fn generate_server_properties(&self, server_dir: &PathBuf) -> anyhow::Result<()> {
        let content = format!(
            r#"# Auto-generado por mc-manager - NO EDITAR MANUALMENTE
# Cambia la configuracion desde la TUI (tecla 'e')

server-port={}
max-players={}
online-mode={}
motd={}
eula=true
"#,
            self.server_port, self.max_players, self.online_mode, self.motd
        );
        fs::write(server_dir.join("server.properties"), content)?;
        Ok(())
    }

    pub fn generate_eula(&self, server_dir: &PathBuf) -> anyhow::Result<()> {
        fs::write(server_dir.join("eula.txt"), "eula=true\n")?;
        Ok(())
    }

    pub fn generate_all(&self, server_dir: &PathBuf) -> anyhow::Result<()> {
        self.generate_start_sh(server_dir)?;
        self.generate_server_properties(server_dir)?;
        self.generate_eula(server_dir)?;
        Ok(())
    }
}

pub fn load_all_server_configs() -> HashMap<String, ServerConfig> {
    let dir = ServerConfig::config_dir();
    let mut configs = HashMap::new();
    if dir.exists() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(config) = serde_json::from_str::<ServerConfig>(&content) {
                                configs.insert(stem.to_string(), config);
                            }
                        }
                    }
                }
            }
        }
    }
    configs
}