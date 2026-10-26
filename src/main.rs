use clap::{Parser, Subcommand};
use Minecraft_Server_Manager_Rust::config::{load_config, save_config, Config, config_path};
use Minecraft_Server_Manager_Rust::daemon::{daemonize, IpcServer, DaemonHandler, IpcClient, IpcRequest, IpcResponse, is_daemon_running, get_daemon_pid, AutoUpdater, DaemonConfig};
use Minecraft_Server_Manager_Rust::tui::run_tui;
use Minecraft_Server_Manager_Rust::daemon::types::UpdateStatus;
use Minecraft_Server_Manager_Rust::server_config;
use std::env;

#[derive(Parser)]
#[command(name = "mc-manager", about = "Minecraft Server Manager - TUI + Daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Ejecuta la TUI interactiva (default)
    Tui,
    /// Inicia el daemon en background
    DaemonStart,
    /// Detiene el daemon
    DaemonStop,
    /// Estado del daemon
    DaemonStatus,
    /// Reinicia el daemon
    DaemonRestart,
    /// Gestiona configuracion
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Lista servidores descubiertos
    List,
}

#[derive(Subcommand)]
enum ConfigAction {
    Show,
    Edit,
    Reset,
    AddDir { path: String },
    RemoveDir { path: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => run_tui_mode(),
        Some(Commands::Tui) => run_tui_mode(),
        Some(Commands::DaemonStart) => daemon_start(),
        Some(Commands::DaemonStop) => daemon_stop(),
        Some(Commands::DaemonStatus) => daemon_status(),
        Some(Commands::DaemonRestart) => daemon_restart(),
        Some(Commands::Config { action }) => config_action(action),
        Some(Commands::List) => list_servers(),
    }
}

fn run_tui_mode() -> anyhow::Result<()> {
    let config = load_config();
    let daemon_config: DaemonConfig = (&config).into();

    if !is_daemon_running(&daemon_config) {
        eprintln!("Advertencia: daemon no esta corriendo. Inicialo con: mc-manager daemon start");
    }

    let app = Minecraft_Server_Manager_Rust::tui::app::App::new(daemon_config);
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run_tui(app))
}

fn daemon_start() -> anyhow::Result<()> {
    let config = load_config();
    let daemon_config: DaemonConfig = (&config).into();

    if is_daemon_running(&daemon_config) {
        println!("Daemon ya esta corriendo");
        return Ok(());
    }

    daemonize(&daemon_config).map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut handler = DaemonHandler::new(daemon_config.clone(), config.search_dirs.clone());
    handler.initialize().map_err(|e| anyhow::anyhow!("Init error: {}", e))?;

    // Auto-update check on daemon start
    let mut updater = AutoUpdater::new(daemon_config.clone());
    if let Err(e) = updater.check_and_prepare() {
        eprintln!("Auto-update check failed: {}", e);
    }

    let ipc_server = IpcServer::new(&daemon_config).map_err(|e| anyhow::anyhow!("IPC error: {}", e))?;

    let handler_fn = move |req| handle_daemon_request(req, &mut handler);

    ipc_server.run(handler_fn).map_err(|e| anyhow::anyhow!("IPC run error: {}", e))
}

fn handle_daemon_request(req: IpcRequest, handler: &mut DaemonHandler) -> IpcResponse {
    match req {
        IpcRequest::ListServers => {
            handler.refresh_servers().ok();
            let servers: Vec<Minecraft_Server_Manager_Rust::daemon::types::ServerInfo> = handler.server_managers.iter()
                .map(|(_id, mgr)| {
                    let mgr = mgr.lock().unwrap();
                    let s = mgr.server();
                    Minecraft_Server_Manager_Rust::daemon::types::ServerInfo {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        path: s.path.display().to_string(),
                        status: s.status.to_string(),
                        service: format!("{:?}", s.service),
                        pid: s.pid,
                    }
                })
                .collect();
            IpcResponse::ServerList { servers }
        }
        IpcRequest::StartServer { server_id } => {
            if let Some(mgr) = handler.server_managers.get(&server_id) {
                let mut mgr = mgr.lock().unwrap();
                match mgr.start() {
                    Ok(_) => IpcResponse::Result { success: true, message: "Iniciado".to_string() },
                    Err(e) => IpcResponse::Error { message: e.to_string() },
                }
            } else {
                IpcResponse::Error { message: "Servidor no encontrado".to_string() }
            }
        }
        IpcRequest::StopServer { server_id } => {
            if let Some(mgr) = handler.server_managers.get(&server_id) {
                let mut mgr = mgr.lock().unwrap();
                match mgr.stop() {
                    Ok(_) => IpcResponse::Result { success: true, message: "Detenido".to_string() },
                    Err(e) => IpcResponse::Error { message: e.to_string() },
                }
            } else {
                IpcResponse::Error { message: "Servidor no encontrado".to_string() }
            }
        }
        IpcRequest::RestartServer { server_id } => {
            if let Some(mgr) = handler.server_managers.get(&server_id) {
                let mut mgr = mgr.lock().unwrap();
                match mgr.restart() {
                    Ok(_) => IpcResponse::Result { success: true, message: "Reiniciado".to_string() },
                    Err(e) => IpcResponse::Error { message: e.to_string() },
                }
            } else {
                IpcResponse::Error { message: "Servidor no encontrado".to_string() }
            }
        }
        IpcRequest::Shutdown => {
            // Apply pending update before shutdown
            let config = load_config();
            let daemon_config: DaemonConfig = (&config).into();
            let updater = AutoUpdater::new(daemon_config);
            if let Err(e) = updater.apply_on_shutdown() {
                eprintln!("Error applying update on shutdown: {}", e);
            }
            IpcResponse::Shutdown
        }
        IpcRequest::ReloadConfig => {
            handler.refresh_servers().ok();
            IpcResponse::Result { success: true, message: "Config recargada".to_string() }
        }
        IpcRequest::CheckUpdates => {
            let config = load_config();
            let daemon_config: DaemonConfig = (&config).into();
            let mut updater = AutoUpdater::new(daemon_config);
            match updater.check_and_prepare() {
                Ok(_) => {
                    let status = updater.check_for_updates_interactive().unwrap_or(UpdateStatus::Error("error".to_string()));
                    match status {
                        UpdateStatus::UpToDate => IpcResponse::CheckUpdatesResponse {
                            current: env!("CARGO_PKG_VERSION").to_string(),
                            latest: env!("CARGO_PKG_VERSION").to_string(),
                            downloaded: false,
                        },
                        UpdateStatus::UpdateAvailable { version, downloaded, .. } => IpcResponse::CheckUpdatesResponse {
                            current: env!("CARGO_PKG_VERSION").to_string(),
                            latest: version,
                            downloaded,
                        },
                        _ => IpcResponse::CheckUpdatesResponse {
                            current: env!("CARGO_PKG_VERSION").to_string(),
                            latest: env!("CARGO_PKG_VERSION").to_string(),
                            downloaded: false,
                        },
                    }
                }
                Err(e) => IpcResponse::Error { message: format!("Error checking updates: {}", e) }
            }
        }
        IpcRequest::ApplyUpdateNow => {
            let config = load_config();
            let daemon_config: DaemonConfig = (&config).into();
            let updater = AutoUpdater::new(daemon_config);
            match updater.apply_now_via_ipc() {
                Ok(_) => IpcResponse::Result { success: true, message: "Actualizacion aplicada".to_string() },
                Err(e) => IpcResponse::Error { message: format!("Error aplicando actualizacion: {}", e) }
            }
        }
        IpcRequest::GetServerConfig { server_id } => {
            let config = server_config::ServerConfig::load(&server_id);
            let config = Minecraft_Server_Manager_Rust::daemon::types::ServerConfig {
                server_id: config.server_id,
                java_path: config.java_path,
                xms: config.xms,
                xmx: config.xmx,
                jvm_extra_args: config.jvm_extra_args,
                server_port: config.server_port,
                max_players: config.max_players,
                online_mode: config.online_mode,
                motd: config.motd,
            };
            IpcResponse::ServerConfig { config }
        }
        IpcRequest::SetServerConfig { config } => {
            let _server_id = config.server_id.clone();
            let config = server_config::ServerConfig {
                server_id: config.server_id,
                java_path: config.java_path,
                xms: config.xms,
                xmx: config.xmx,
                jvm_extra_args: config.jvm_extra_args,
                server_port: config.server_port,
                max_players: config.max_players,
                online_mode: config.online_mode,
                motd: config.motd,
            };
            let server_id = config.server_id.clone();
            // Find the server path from the handler
            if let Some(mgr) = handler.server_managers.get(&server_id) {
                let mgr = mgr.lock().unwrap();
                let server = mgr.server();
                let server_dir = &server.path;
                if let Err(e) = config.generate_all(server_dir) {
                    IpcResponse::Error { message: format!("Error generando config: {}", e) }
                } else if let Err(e) = config.save() {
                    IpcResponse::Error { message: format!("Error guardando config: {}", e) }
                } else {
                    IpcResponse::Result { success: true, message: "Configuracion guardada. Cambios aplican en proximo reinicio".to_string() }
                }
            } else {
                IpcResponse::Error { message: "Servidor no encontrado".to_string() }
            }
        }
        _ => IpcResponse::Error { message: "Comando no implementado".to_string() }
    }
}

fn daemon_stop() -> anyhow::Result<()> {
    let config = load_config();
    let daemon_config: DaemonConfig = (&config).into();

    if !is_daemon_running(&daemon_config) {
        println!("Daemon no esta corriendo");
        return Ok(());
    }

    let client = IpcClient::new(&daemon_config);
    match client.send(IpcRequest::Shutdown) {
        Ok(_) => println!("Daemon detenido"),
        Err(e) => eprintln!("Error deteniendo daemon: {}", e),
    }
    Ok(())
}

fn daemon_status() -> anyhow::Result<()> {
    let config = load_config();
    let daemon_config: DaemonConfig = (&config).into();

    if is_daemon_running(&daemon_config) {
        if let Some(pid) = get_daemon_pid(&daemon_config) {
            println!("Daemon corriendo (PID: {})", pid);
            println!("Socket: {}", daemon_config.socket_file.display());
        }
    } else {
        println!("Daemon no esta corriendo");
    }
    Ok(())
}

fn daemon_restart() -> anyhow::Result<()> {
    daemon_stop()?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    daemon_start()
}

fn config_action(action: ConfigAction) -> anyhow::Result<()> {
    let mut config = load_config();

    match action {
        ConfigAction::Show => {
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
        ConfigAction::Edit => {
            println!("Config file: {}", config_path().display());
            println!("Usa tu editor preferido para editarlo");
        }
        ConfigAction::Reset => {
            config = Config::default();
            save_config(&config);
            println!("Configuracion restablecida");
        }
        ConfigAction::AddDir { path } => {
            let p = std::path::PathBuf::from(path);
            if p.exists() && !config.search_dirs.contains(&p) {
                config.search_dirs.push(p);
                save_config(&config);
                println!("Directorio agregado");
            } else {
                eprintln!("Directorio invalido o ya existe");
            }
        }
        ConfigAction::RemoveDir { path } => {
            let p = std::path::PathBuf::from(path);
            config.search_dirs.retain(|d| d != &p);
            save_config(&config);
            println!("Directorio removido");
        }
    }
    Ok(())
}

fn list_servers() -> anyhow::Result<()> {
    let config = load_config();
    let daemon_config: DaemonConfig = (&config).into();

    if !is_daemon_running(&daemon_config) {
        eprintln!("Daemon no esta corriendo. Inicialo con: mc-manager daemon start");
        return Ok(());
    }

    let client = IpcClient::new(&daemon_config);
    match client.send(IpcRequest::ListServers) {
        Ok(IpcResponse::ServerList { servers }) => {
            if servers.is_empty() {
                println!("No hay servidores");
            } else {
                for s in servers {
                    println!("{} [{}] - {} ({})", s.name, s.status, s.path, s.service);
                }
            }
        }
        Ok(IpcResponse::Error { message }) => {
            eprintln!("Error: {}", message);
        }
        Err(e) => {
            eprintln!("Error conectando al daemon: {}", e);
        }
        _ => eprintln!("Respuesta inesperada"),
    }
    Ok(())
}