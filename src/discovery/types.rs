use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Estado de un servidor Minecraft
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    /// Servidor apagado
    Stopped,
    /// Servidor en ejecución
    Running,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStatus::Stopped => write!(f, "stopped"),
            ServerStatus::Running => write!(f, "running"),
        }
    }
}

/// Tipo de modloader/servicio de Minecraft
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ServerService {
    #[default]
    Vanilla,
    Paper,
    Spigot,
    Forge,
    Fabric,
    NeoForge,
    /// No se pudo determinar el servicio
    Unknown,
}

impl ServerService {
    /// Identifica el servicio basándose en archivos presentes en el directorio
    pub fn detect_from_dir(dir: &std::path::Path) -> Self {
        // NeoForge: carpeta neoforge/ o neoforge.toml
        if dir.join("neoforge").is_dir() || dir.join("neoforge.toml").exists() {
            return ServerService::NeoForge;
        }
        // Forge: carpeta forge/ o forge-mods/
        if dir.join("forge").is_dir() || dir.join("forge-mods").is_dir() {
            return ServerService::Forge;
        }
        // Fabric: fabric.mod.json
        if dir.join("fabric.mod.json").exists() {
            return ServerService::Fabric;
        }
        // Paper: paper.yml, paper-config.yml
        if dir.join("paper.yml").exists() || dir.join("paper-config").is_dir() {
            return ServerService::Paper;
        }
        // Spigot: spigot.yml
        if dir.join("spigot.yml").exists() {
            return ServerService::Spigot;
        }
        // Vanilla: server.jar sin mod files
        if dir.join("server.jar").exists() {
            return ServerService::Vanilla;
        }
        ServerService::Unknown
    }
}

/// Información de un servidor Minecraft detectado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Identificador único (path like string)
    pub id: String,
    /// Nombre del servidor (último componente del path)
    pub name: String,
    /// Ruta al directorio del servidor
    pub path: PathBuf,
    /// Estado actual
    pub status: ServerStatus,
    /// Tipo de servicio/modloader
    pub service: ServerService,
    /// PID del proceso si está corriendo
    pub pid: Option<u32>,
}

impl ServerInfo {
    /// Crea una nueva ServerInfo validando los datos
    pub fn new(path: PathBuf, status: ServerStatus, service: ServerService, pid: Option<u32>) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());
        let id = path.to_string_lossy().to_string();
        ServerInfo {
            id,
            name,
            path,
            status,
            service,
            pid,
        }
    }
}

/// Configuración del módulo de discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Directorios donde buscar servidores
    pub search_dirs: Vec<PathBuf>,
    /// Ruta al ejecutable Java (opcional, por defecto "java")
    pub java_path: Option<String>,
    /// Timeout en milisegundos para el escaneo
    pub timeout_ms: Option<u64>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            search_dirs: vec![],
            java_path: None,
            timeout_ms: Some(5000),
        }
    }
}

impl DiscoveryConfig {
    /// Crea una configuración vacía
    pub fn new() -> Self {
        DiscoveryConfig::default()
    }
}