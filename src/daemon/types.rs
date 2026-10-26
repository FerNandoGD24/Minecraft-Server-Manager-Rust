use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuracion por servidor (generada y gestionada por el manager)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Configuracion del daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Directorio base para PID/socket/logs
    pub base_dir: PathBuf,
    /// Archivo PID
    pub pid_file: PathBuf,
    /// Socket Unix
    pub socket_file: PathBuf,
    /// Auto-restart servidores caidos
    pub auto_restart: bool,
    /// Intervalo check auto-restart (segundos)
    pub restart_check_interval: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mc-manager");
        
        DaemonConfig {
            base_dir: base.clone(),
            pid_file: base.join("daemon.pid"),
            socket_file: base.join("daemon.sock"),
            auto_restart: false,
            restart_check_interval: 30,
        }
    }
}

/// Estado del daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub running: bool,
    pub pid: Option<u32>,
    pub started_at: Option<u64>,
    pub servers_managed: usize,
}

/// Mensajes IPC Cliente -> Daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    ListServers,
    StartServer { server_id: String },
    StopServer { server_id: String },
    RestartServer { server_id: String },
    GetMetrics { server_id: String },
    AttachConsole { server_id: String },
    SendCommand { server_id: String, command: String },
    GetServerInfo { server_id: String },
    Shutdown,
    ReloadConfig,
    CheckUpdates,
    ApplyUpdateNow,
    GetServerConfig { server_id: String },
    SetServerConfig { config: ServerConfig },
}

/// Respuestas IPC Daemon -> Cliente
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    ServerList { servers: Vec<ServerInfo> },
    Result { success: bool, message: String },
    Metrics { cpu_percent: f32, memory_rss: u64, disk_read_bps: u64, disk_write_bps: u64 },
    ConsoleOutput { line: String },
    ServerInfo { id: String, name: String, path: String, status: String, service: String },
    Error { message: String },
    Shutdown,
    CheckUpdatesResponse { current: String, latest: String, downloaded: bool },
    ServerConfig { config: ServerConfig },
}

/// Info de servidor para IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub status: String,
    pub service: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    pub pending: bool,
    pub version: String,
    pub method: String,
    pub temp_path: String,
    pub downloaded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMarker {
    pub updated: bool,
    pub version: String,
    pub method: String,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable { version: String, download_url: String, downloaded: bool },
    Error(String),
}