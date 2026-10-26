use std::path::PathBuf;

/// Acciones de control disponibles para un servidor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Start,
    Stop,
    Restart,
}

/// Resultado de una operación de control
#[derive(Debug, Clone)]
pub struct ControlResult {
    pub action: ControlAction,
    pub success: bool,
    pub message: String,
    pub pid: Option<u32>,
}

impl ControlResult {
    pub fn success(action: ControlAction, pid: Option<u32>, message: &str) -> Self {
        ControlResult {
            action,
            success: true,
            message: message.to_string(),
            pid,
        }
    }

    pub fn failure(action: ControlAction, message: &str) -> Self {
        ControlResult {
            action,
            success: false,
            message: message.to_string(),
            pid: None,
        }
    }
}

/// Errores de control de servidores
#[derive(Debug, Clone)]
pub enum ControlError {
    AlreadyRunning,
    AlreadyStopped,
    ServerNotFound(PathBuf),
    ScriptNotFound(PathBuf),
    ProcessSpawnFailed(String),
    CommandWriteFailed(String),
    ShutdownTimeout(u32),
    IoError(String),
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::AlreadyRunning => write!(f, "Servidor ya está corriendo"),
            ControlError::AlreadyStopped => write!(f, "Servidor ya está parado"),
            ControlError::ServerNotFound(p) => write!(f, "Servidor no encontrado: {}", p.display()),
            ControlError::ScriptNotFound(p) => write!(f, "Script no encontrado: {}", p.display()),
            ControlError::ProcessSpawnFailed(e) => write!(f, "Error iniciando proceso: {}", e),
            ControlError::CommandWriteFailed(e) => write!(f, "Error escribiendo comando: {}", e),
            ControlError::ShutdownTimeout(pid) => write!(f, "Timeout apagando proceso PID {}", pid),
            ControlError::IoError(e) => write!(f, "Error de E/S: {}", e),
        }
    }
}

impl std::error::Error for ControlError {}

impl From<std::io::Error> for ControlError {
    fn from(e: std::io::Error) -> Self {
        ControlError::IoError(e.to_string())
    }
}

impl From<String> for ControlError {
    fn from(e: String) -> Self {
        ControlError::IoError(e)
    }
}

impl From<&str> for ControlError {
    fn from(e: &str) -> Self {
        ControlError::IoError(e.to_string())
    }
}