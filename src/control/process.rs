use std::io::Write;
use std::process::{Child, Command, Stdio};

/// Wrapper de un proceso hijo con capacidad de escribir en stdin y enviar señales
pub struct ManagedChild {
    child: Child,
}

impl ManagedChild {
    /// Inicia un proceso hijo con stdin pipe para poder enviar comandos
    pub fn spawn(mut command: Command) -> std::io::Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command.spawn()?;
        Ok(ManagedChild { child })
    }

    /// ID del proceso
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Escribe un comando + newline al stdin del proceso
    pub fn write_command(&mut self, cmd: &str) -> Result<(), String> {
        if let Some(ref mut stdin) = self.child.stdin {
            writeln!(stdin, "{}", cmd)
                .map_err(|e| format!("Error escribiendo a stdin: {}", e))?;
            stdin
                .flush()
                .map_err(|e| format!("Error flush stdin: {}", e))?;
            Ok(())
        } else {
            Err("Stdin no disponible".to_string())
        }
    }

    /// Verifica si el proceso sigue vivo
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Espera a que el proceso termine (no blocking si no ha terminado)
    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    /// Espera bloqueante a que el proceso termine
    pub fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait()
    }

    /// Envia señal SIGTERM para shutdown graceful
    #[cfg(target_os = "linux")]
    pub fn terminate(&self) -> Result<(), String> {
        let pid = self.child.id() as i32;
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), nix::sys::signal::Signal::SIGTERM)
            .map_err(|e| format!("Error enviando SIGTERM: {}", e))?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn terminate(&self) -> Result<(), String> {
        // Fallback: kill command
        let pid = self.child.id();
        std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .map_err(|e| format!("Error kill: {}", e))?;
        Ok(())
    }

    /// Envia señal SIGKILL (forzado)
    #[cfg(target_os = "linux")]
    pub fn kill(&self) -> Result<(), String> {
        let pid = self.child.id() as i32;
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), nix::sys::signal::Signal::SIGKILL)
            .map_err(|e| format!("Error enviando SIGKILL: {}", e))?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn kill(&self) -> Result<(), String> {
        let pid = self.child.id();
        std::process::Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status()
            .map_err(|e| format!("Error kill: {}", e))?;
        Ok(())
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        // Intentar terminate graceful al destructor
        let _ = self.terminate();
        let _ = self.wait();
    }
}