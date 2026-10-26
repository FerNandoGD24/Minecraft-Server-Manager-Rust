use crate::discovery::types::{ServerInfo, ServerStatus};
use crate::control::process::ManagedChild;
use crate::control::types::{ControlAction, ControlError, ControlResult};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, ControlError>;

const STOP_GRACEFUL_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_FORCE_TIMEOUT: Duration = Duration::from_secs(10);

/// Scripts de arranque conocidos
const START_SCRIPTS: &[&str] = &["start.sh", "start.bat", "run.sh", "run.bat"];

/// Manager para controlar un servidor Minecraft
pub struct ServerManager {
    server: ServerInfo,
    child: Option<ManagedChild>,
}

impl ServerManager {
    /// Crea un nuevo manager para el servidor dado
    pub fn new(server: ServerInfo) -> Self {
        ServerManager { server, child: None }
    }

    /// Obtiene información del servidor
    pub fn server(&self) -> &ServerInfo {
        &self.server
    }

    /// Inicia el servidor
    pub fn start(&mut self) -> Result<ControlResult> {
        // Verificar si ya está corriendo
        if self.server.status == ServerStatus::Running {
            return Ok(ControlResult::failure(
                ControlAction::Start,
                "El servidor ya está corriendo",
            ));
        }

        let server_dir = &self.server.path;

        // Buscar script de arranque
        let script_path = Self::find_start_script(server_dir);
        let mut command = if let Some(script) = script_path {
            let mut cmd = Command::new(script);
            cmd.current_dir(server_dir);
            cmd
        } else {
            // Fallback: java -jar server.jar
            let jar = server_dir.join("server.jar");
            if !jar.exists() {
                return Err(ControlError::ServerNotFound(server_dir.to_path_buf()));
            }
            let mut cmd = Command::new("java");
            cmd.args(["-jar", "server.jar", "nogui"]);
            cmd.current_dir(server_dir);
            cmd
        };

        // Spawn proceso
        let managed = ManagedChild::spawn(command)
            .map_err(|e| ControlError::ProcessSpawnFailed(e.to_string()))?;

        let pid = managed.id();
        self.child = Some(managed);

        Ok(ControlResult::success(
            ControlAction::Start,
            Some(pid),
            "Servidor iniciado correctamente",
        ))
    }

    /// Detiene el servidor enviando comando 'stop' a la consola
    pub fn stop(&mut self) -> Result<ControlResult> {
        // Verificar si ya está parado
        if self.server.status == ServerStatus::Stopped && self.child.is_none() {
            return Ok(ControlResult::failure(
                ControlAction::Stop,
                "El servidor ya está parado",
            ));
        }

        // Si no tenemos el child pero dice running, no podemos hacer mucho
        let Some(ref mut child) = self.child else {
            return Err(ControlError::AlreadyStopped);
        };

        // Intentar shutdown graceful enviando 'stop' a la consola
        let _ = child.write_command("stop");

        // Esperar graceful shutdown
        let start = Instant::now();
        while start.elapsed() < STOP_GRACEFUL_TIMEOUT {
            if let Ok(Some(_)) = child.try_wait() {
                // Proceso terminó
                self.child = None;
                return Ok(ControlResult::success(
                    ControlAction::Stop,
                    None,
                    "Servidor detenido correctamente (comando stop)",
                ));
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        // Timeout graceful: enviar SIGTERM
        eprintln!("Graceful timeout, enviando SIGTERM a PID {}", child.id());
        child.terminate()?;

        // Esperar un poco más
        let start = Instant::now();
        while start.elapsed() < STOP_FORCE_TIMEOUT {
            if let Ok(Some(_)) = child.try_wait() {
                self.child = None;
                return Ok(ControlResult::success(
                    ControlAction::Stop,
                    None,
                    "Servidor detenido (SIGTERM)",
                ));
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        // Forzar con SIGKILL
        eprintln!("SIGTERM timeout, enviando SIGKILL a PID {}", child.id());
        child.kill()?;

        // Esperar final
        let _ = child.wait();
        self.child = None;

        Ok(ControlResult::success(
            ControlAction::Stop,
            None,
            "Servidor detenido forzadamente (SIGKILL)",
        ))
    }

    /// Reinicia el servidor (stop + start)
    pub fn restart(&mut self) -> Result<ControlResult> {
        // Stop
        let stop_result = self.stop();
        if !stop_result.is_ok() && !matches!(stop_result, Err(ControlError::AlreadyStopped)) {
            return stop_result;
        }

        // Start
        self.start()
    }

    /// Envía un comando arbitrario a la consola del servidor
    pub fn send_command(&mut self, cmd: &str) -> Result<()> {
        let Some(ref mut child) = self.child else {
            return Err(ControlError::AlreadyStopped);
        };

        child.write_command(cmd)?;
        Ok(())
    }

    /// Busca un script de arranque en el directorio del servidor
    fn find_start_script(server_dir: &Path) -> Option<std::path::PathBuf> {
        for script in START_SCRIPTS {
            let path = server_dir.join(script);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }
}