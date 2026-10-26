use crate::discovery::types::{ServerInfo, ServerService, ServerStatus};
use std::path::{Path, PathBuf};

/// Enumera procesos Java en ejecucion inspeccionando /proc (Linux)
/// Devuelve una lista de (pid, working_dir, cmdline)
#[cfg(target_os = "linux")]
fn enumerate_java_processes() -> Vec<(u32, PathBuf, Vec<String>)> {
    let mut result = Vec::new();

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return result,
    };

    for entry in proc_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Solo numeros (PIDs)
        if !entry.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        // Leer cmdline
        let cmdline_path = path.join("cmdline");
        let cmdline_bytes = match std::fs::read(&cmdline_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if cmdline_bytes.is_empty() {
            continue;
        }

        // cmdline es null-separated
        let cmdline: Vec<String> = cmdline_bytes
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .filter_map(|s| std::str::from_utf8(s).ok().map(|s| s.to_string()))
            .collect();

        if cmdline.is_empty() {
            continue;
        }

        // Verificar que sea un proceso Java
        let first = &cmdline[0];
        let is_java = first == "java" || first.ends_with("/java") || first.contains("javaw");
        if !is_java {
            continue;
        }

        // Obtener PID
        let pid: u32 = match entry.file_name().to_string_lossy().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Obtener working directory del proceso
        let cwd_path = path.join("cwd");
        let working_dir = match std::fs::read_link(&cwd_path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        result.push((pid, working_dir, cmdline));
    }

    result
}

#[cfg(not(target_os = "linux"))]
fn enumerate_java_processes() -> Vec<(u32, PathBuf, Vec<String>)> {
    // Implementacion placeholder para otros SO
    // Se puede extender con `ps` command
    Vec::new()
}

/// Verifica si un directorio contiene archivos tipicos de servidores Minecraft
fn is_server_directory(dir: &Path) -> bool {
    // server.jar es el indicador mas comun
    if dir.join("server.jar").exists() {
        return true;
    }
    // Scripts de arranque comunes
    if dir.join("start.sh").exists()
        || dir.join("start.bat").exists()
        || dir.join("run.sh").exists()
        || dir.join("run.bat").exists()
    {
        return true;
    }
    // Archivos de identificacion de modloaders
    if dir.join("bukkit.yml").exists()
        || dir.join("spigot.yml").exists()
        || dir.join("paper.yml").exists()
        || dir.join("fabric.mod.json").exists()
        || dir.join("neoforge.toml").exists()
        || dir.join("forge").is_dir()
        || dir.join("forge-mods").is_dir()
        || dir.join("neoforge").is_dir()
    {
        return true;
    }
    false
}

/// Detecta servidores corriendo mediante procesos Java
fn detect_running_servers() -> Vec<ServerInfo> {
    let processes = enumerate_java_processes();
    let mut servers = Vec::new();

    for (pid, working_dir, cmdline) in processes {
        // Verificar que el proceso este relacionado con Minecraft
        let is_minecraft = cmdline.iter().any(|arg| {
            arg.contains("server.jar")
                || arg.contains("nogui")
                || arg.contains("spigot")
                || arg.contains("paper")
                || arg.contains("forge")
                || arg.contains("fabric")
                || arg.contains("neoforge")
                || arg.contains("bukkit")
        });

        if !is_minecraft && !is_server_directory(&working_dir) {
            continue;
        }

        // Detectar servicio
        let service = ServerService::detect_from_dir(&working_dir);

        servers.push(ServerInfo::new(
            working_dir,
            ServerStatus::Running,
            service,
            Some(pid),
        ));
    }

    servers
}

/// Detecta servidores apagados en directorios configurados
fn detect_stopped_servers(search_dirs: &[PathBuf]) -> Vec<ServerInfo> {
    let mut servers = Vec::new();

    for search_dir in search_dirs {
        if !search_dir.exists() {
            eprintln!("Directorio no existe: {}", search_dir.display());
            continue;
        }

        if !search_dir.is_dir() {
            continue;
        }

        match std::fs::read_dir(search_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }

                    if is_server_directory(&path) {
                        let service = ServerService::detect_from_dir(&path);
                        servers.push(ServerInfo::new(
                            path,
                            ServerStatus::Stopped,
                            service,
                            None,
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!("Error leyendo {}: {}", search_dir.display(), e);
            }
        }
    }

    servers
}

/// Combina deteccion de servidores running y stopped
/// Si un directorio aparece en ambos, se prefiere el estado Running
pub fn scan_servers(search_dirs: &[PathBuf]) -> Vec<ServerInfo> {
    let running = detect_running_servers();
    let stopped = detect_stopped_servers(search_dirs);

    // Usar un HashMap-like para deduplicar por path
    // Running prevalece sobre stopped
    let mut result: Vec<ServerInfo> = running;

    for stopped_server in stopped {
        // Solo agregar si no hay un running con el mismo path
        let already_running = result.iter().any(|s| s.path == stopped_server.path);
        if !already_running {
            result.push(stopped_server);
        }
    }

    // Ordenar por nombre
    result.sort_by(|a, b| a.name.cmp(&b.name));

    result
}