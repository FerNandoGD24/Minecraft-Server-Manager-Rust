use crate::daemon::types::DaemonConfig;
use nix::unistd::{fork, setsid, ForkResult};
use std::fs;
use std::os::unix::io::AsRawFd;
use std::process;

/// Realiza el doble fork para daemonizar el proceso
/// Retorna Ok(()) en el proceso daemon, nunca retorna en el parent original
pub fn daemonize(config: &DaemonConfig) -> Result<(), String> {
    // Crear directorio base
    fs::create_dir_all(&config.base_dir)
        .map_err(|e| format!("Error creando directorio base: {}", e))?;

    // Fork #1
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // Parent original sale
            process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Child continua
        }
        Err(e) => return Err(format!("Fork #1 fallo: {}", e)),
    }

    // Nueva sesion (setsid) - child se convierte en session leader
    setsid().map_err(|e| format!("setsid fallo: {}", e))?;

    // Fork #2 - para asegurar que no somos session leader
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // Parent del segundo fork sale
            process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Child final - el daemon real
        }
        Err(e) => return Err(format!("Fork #2 fallo: {}", e)),
    }

    // Cambiar directorio de trabajo
    std::env::set_current_dir(&config.base_dir)
        .map_err(|e| format!("Error cambiando directorio: {}", e))?;

    // Umask permisivo
    unsafe { libc::umask(0o027) };

    // Cerrar FDs estandar y redirigir a /dev/null
    redirect_stdio()?;

    // Escribir PID file
    write_pid_file(config)?;

    Ok(())
}

/// Redirige stdin, stdout, stderr a /dev/null
fn redirect_stdio() -> Result<(), String> {
    let devnull = fs::File::options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|e| format!("Error abriendo /dev/null: {}", e))?;

    let fd = devnull.as_raw_fd();

    // dup2 para cada fd estandar
    unsafe {
        libc::dup2(fd, 0); // stdin
        libc::dup2(fd, 1); // stdout
        libc::dup2(fd, 2); // stderr
    }

    // Mantener devnull vivo
    std::mem::forget(devnull);
    Ok(())
}

/// Escribe el PID file
fn write_pid_file(config: &DaemonConfig) -> Result<(), String> {
    let pid = process::id();
    let pid_str = pid.to_string();
    
    fs::write(&config.pid_file, pid_str)
        .map_err(|e| format!("Error escribiendo PID file: {}", e))?;
    
    Ok(())
}

/// Elimina el PID file
pub fn remove_pid_file(config: &DaemonConfig) -> Result<(), String> {
    if config.pid_file.exists() {
        fs::remove_file(&config.pid_file)
            .map_err(|e| format!("Error eliminando PID file: {}", e))?;
    }
    Ok(())
}

/// Verifica si el daemon ya esta corriendo
pub fn is_daemon_running(config: &DaemonConfig) -> bool {
    if !config.pid_file.exists() {
        return false;
    }

    let pid_str = match fs::read_to_string(&config.pid_file) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return false,
    };

    let pid: u32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Verificar si el proceso existe (enviar signal 0)
    match nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid as i32),
        nix::sys::signal::Signal::SIGCONT, // signal 0 no existe en nix, usamos SIGCONT que no hace nada
    ) {
        Ok(_) => true,
        Err(nix::errno::Errno::ESRCH) => false, // No such process
        Err(_) => false,
    }
}

/// Obtiene el PID del daemon corriendo
pub fn get_daemon_pid(config: &DaemonConfig) -> Option<u32> {
    if !config.pid_file.exists() {
        return None;
    }
    
    let pid_str = fs::read_to_string(&config.pid_file).ok()?;
    pid_str.trim().parse().ok()
}