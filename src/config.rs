use crate::console::types::{ConsoleConfig, DetachSignal};
use crate::daemon::types::DaemonConfig;
use crate::discovery::types::DiscoveryConfig;
use dirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuración global persistente
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directorios donde buscar servidores Minecraft
    pub search_dirs: Vec<PathBuf>,
    /// Configuración del daemon
    pub daemon: DaemonConfigSection,
    /// Configuración de la TUI
    pub tui: TuiConfigSection,
    /// Configuración de la consola
    pub console: ConsoleConfigSection,
}

/// Sección daemon en config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfigSection {
    pub auto_restart: bool,
    pub restart_check_interval: u64,
}

/// Sección TUI en config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfigSection {
    pub theme: String,
    pub show_metrics: bool,
    pub poll_interval_ms: u64,
}

/// Sección console en config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleConfigSection {
    pub detach_signal: String,
    pub history_lines: usize,
    pub show_timestamp: bool,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let default_dirs = vec![
            home.join("minecraft-servers"),
            PathBuf::from("/opt/minecraft"),
            PathBuf::from("/var/lib/minecraft"),
        ];

        Config {
            search_dirs: default_dirs.into_iter().filter(|p| p.exists()).collect(),
            daemon: DaemonConfigSection::default(),
            tui: TuiConfigSection::default(),
            console: ConsoleConfigSection::default(),
        }
    }
}

impl Default for DaemonConfigSection {
    fn default() -> Self {
        DaemonConfigSection {
            auto_restart: false,
            restart_check_interval: 30,
        }
    }
}

impl Default for TuiConfigSection {
    fn default() -> Self {
        TuiConfigSection {
            theme: "default".to_string(),
            show_metrics: true,
            poll_interval_ms: 2000,
        }
    }
}

impl Default for ConsoleConfigSection {
    fn default() -> Self {
        ConsoleConfigSection {
            detach_signal: "ctrl_backslash".to_string(),
            history_lines: 1000,
            show_timestamp: false,
        }
    }
}

/// Ruta al archivo de configuración
pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mc-manager")
        .join("config.json")
}

/// Carga configuración desde config.json
/// Si no existe, crea una por defecto y la guarda
pub fn load_config() -> Config {
    let path = config_path();

    if !path.exists() {
        let default = Config::default();
        save_config(&default);
        return default;
    }

    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            match serde_json::from_str::<Config>(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("⚠ Error parseando config.json: {}. Usando defaults.", e);
                    Config::default()
                }
            }
        }
        Err(e) => {
            eprintln!("⚠ Error leyendo config.json: {}. Usando defaults.", e);
            Config::default()
        }
    }
}

/// Guarda configuración en config.json
pub fn save_config(config: &Config) -> bool {
    let path = config_path();

    // Crear directorio si no existe
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Error creando directorio config: {}", e);
            return false;
        }
    }

    match serde_json::to_string_pretty(config) {
        Ok(json) => match std::fs::write(&path, json) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Error guardando config.json: {}", e);
                false
            }
        },
        Err(e) => {
            eprintln!("Error serializando config: {}", e);
            false
        }
    }
}

/// Convierte Config a DiscoveryConfig
impl From<&Config> for DiscoveryConfig {
    fn from(config: &Config) -> Self {
        DiscoveryConfig {
            search_dirs: config.search_dirs.clone(),
            java_path: None,
            timeout_ms: Some(5000),
        }
    }
}

/// Convierte Config a DaemonConfig
impl From<&Config> for DaemonConfig {
    fn from(config: &Config) -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mc-manager");

        DaemonConfig {
            base_dir: base.clone(),
            pid_file: base.join("daemon.pid"),
            socket_file: base.join("daemon.sock"),
            auto_restart: config.daemon.auto_restart,
            restart_check_interval: config.daemon.restart_check_interval,
        }
    }
}

/// Convierte Config a ConsoleConfig
impl From<&Config> for ConsoleConfig {
    fn from(config: &Config) -> Self {
        let detach_signal = match config.console.detach_signal.as_str() {
            "ctrl_d" => DetachSignal::CtrlD,
            "f12" => DetachSignal::F12,
            _ => DetachSignal::CtrlBackslash,
        };

        ConsoleConfig {
            detach_signal,
            history_lines: config.console.history_lines,
            show_timestamp: config.console.show_timestamp,
        }
    }
}