use crate::daemon::{DaemonConfig, IpcClient, IpcRequest, IpcResponse, ServerInfo};
use crate::monitoring::ResourceMetrics;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task;
use crate::daemon::version_tracker::fetch_changelog_for_version;

/// Modos de la aplicacion
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Dashboard,
    Console { server_id: String },
    Help,
    ConfirmAction { action: ServerAction, server_id: String },
    UpdateAvailable { version: String, download_url: String, downloaded: bool },
    Changelog { version: String, content: String },
    ServerConfig { server_id: String, field: usize, editing: bool, config: crate::daemon::types::ServerConfig },
}

/// Acciones de servidor para confirmacion
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerAction {
    Start,
    Stop,
    Restart,
}

/// Estado principal de la aplicacion
pub struct App {
    pub client: IpcClient,
    pub servers: Vec<ServerInfo>,
    pub selected: usize,
    pub mode: AppMode,
    pub console_session_id: Option<u32>,
    pub console_server_id: Option<String>,
    pub metrics: HashMap<String, ResourceMetrics>,
    pub last_metrics_poll: Instant,
    pub should_quit: bool,
    pub show_help: bool,
    pub status_message: String,
    pub daemon_connected: bool,
    pub logs: Vec<String>,
}

impl App {
    pub fn new(config: DaemonConfig) -> Self {
        let client = IpcClient::new(&config);

        App {
            client,
            servers: Vec::new(),
            selected: 0,
            mode: AppMode::Dashboard,
            console_session_id: None,
            console_server_id: None,
            metrics: HashMap::new(),
            last_metrics_poll: Instant::now(),
            should_quit: false,
            show_help: false,
            status_message: "Conectando al daemon...".to_string(),
            daemon_connected: false,
            logs: Vec::new(),
        }
    }

    /// Inicializa la conexion y carga servidores
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        self.refresh_servers().await?;
        self.start_metrics_polling().await;
        Ok(())
    }

    /// Refresca la lista de servidores desde el daemon
    pub async fn refresh_servers(&mut self) -> anyhow::Result<()> {
        match self.client.send(IpcRequest::ListServers) {
            Ok(IpcResponse::ServerList { servers }) => {
                self.servers = servers;
                self.daemon_connected = true;
                self.status_message = format!("{} servidores encontrados", self.servers.len());
                if self.selected >= self.servers.len() && !self.servers.is_empty() {
                    self.selected = self.servers.len() - 1;
                }
            }
            Ok(_) => {
                self.daemon_connected = false;
                self.status_message = "Respuesta inesperada del daemon".to_string();
            }
            Err(e) => {
                self.daemon_connected = false;
                self.status_message = format!("Error conectando al daemon: {}", e);
            }
        }
        Ok(())
    }

    /// Inicia task de polling de metricas cada 2s
    async fn start_metrics_polling(&mut self) {
        let client = self.client.clone();
        let (tx, mut rx) = mpsc::channel::<HashMap<String, ResourceMetrics>>(32);

        task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let mut all_metrics = HashMap::new();

                if let Ok(IpcResponse::ServerList { servers }) = client.send(IpcRequest::ListServers) {
                    for server in servers {
                        if let Some(pid) = server.pid {
                        }
                    }
                }

                let _ = tx.send(all_metrics).await;
            }
        });

        task::spawn(async move {
            while let Some(metrics) = rx.recv().await {
            }
        });
    }

    /// Maneja eventos de teclado
    pub async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.code == KeyCode::Char('q') || (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
        }

        if key.code == KeyCode::Char('?') {
            self.mode = AppMode::Help;
            return Ok(());
        }

        match &self.mode {
            AppMode::Dashboard => self.handle_dashboard_key(key).await,
            AppMode::Console { server_id } => self.handle_console_key(key, server_id.clone()).await,
            AppMode::Help => self.handle_help_key(key),
            AppMode::ConfirmAction { action, server_id } => self.handle_confirm_key(key, *action, server_id.clone()).await,
            AppMode::UpdateAvailable { version, .. } => self.handle_update_available_key(key, version.clone()).await,
            AppMode::Changelog { .. } => self.handle_changelog_key(key),
            AppMode::ServerConfig { .. } => self.handle_server_config_key(key).await,
        }
    }

    async fn handle_dashboard_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if self.servers.is_empty() {
            return Ok(());
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.servers.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Enter => {
                let server = &self.servers[self.selected];
                self.attach_console(server.id.clone()).await?;
            }
            KeyCode::Char('s') => {
                let server = &self.servers[self.selected];
                if server.status == "stopped" {
                    self.mode = AppMode::ConfirmAction {
                        action: ServerAction::Start,
                        server_id: server.id.clone(),
                    };
                }
            }
            KeyCode::Char('x') => {
                let server = &self.servers[self.selected];
                if server.status == "running" {
                    self.mode = AppMode::ConfirmAction {
                        action: ServerAction::Stop,
                        server_id: server.id.clone(),
                    };
                }
            }
            KeyCode::Char('r') => {
                let server = &self.servers[self.selected];
                self.mode = AppMode::ConfirmAction {
                    action: ServerAction::Restart,
                    server_id: server.id.clone(),
                };
            }
            KeyCode::Char('u') => {
                self.check_updates().await?;
            }
            KeyCode::Char('e') => {
                let server = &self.servers[self.selected];
                self.load_server_config(server.id.clone()).await?;
            }
            KeyCode::Char('m') => {
            }
            KeyCode::F(5) => {
                self.refresh_servers().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_console_key(&mut self, key: KeyEvent, server_id: String) -> anyhow::Result<()> {
        if key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::CONTROL {
            self.detach_console().await?;
            return Ok(());
        }

        let cmd = self.key_to_command(key);
        if !cmd.is_empty() {
            let request = IpcRequest::SendCommand {
                server_id: server_id.clone(),
                command: cmd,
            };
            if let Err(e) = self.client.send(request) {
                self.logs.push(format!("[ERROR] {}", e));
            }
        }
        Ok(())
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.code == KeyCode::Char('?') || key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
            self.mode = AppMode::Dashboard;
        }
        Ok(())
    }

    async fn handle_confirm_key(&mut self, key: KeyEvent, action: ServerAction, server_id: String) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.execute_server_action(action, &server_id).await?;
                self.mode = AppMode::Dashboard;
            }
            KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = AppMode::Dashboard;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_update_available_key(&mut self, key: KeyEvent, version: String) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Char('c') => {
                let changelog = self.fetch_changelog(&version).await.unwrap_or_else(|_| "No disponible".to_string());
                self.mode = AppMode::Changelog { version, content: changelog };
            }
            KeyCode::Char('d') => {
                self.status_message = "Descargando actualizacion...".to_string();
                match self.client.send(IpcRequest::ApplyUpdateNow) {
                    Ok(IpcResponse::Result { success, message }) => {
                        self.status_message = message;
                        if success {
                            self.mode = AppMode::Dashboard;
                        }
                    }
                    Ok(IpcResponse::Error { message }) => {
                        self.status_message = format!("Error: {}", message);
                    }
                    Err(e) => {
                        self.status_message = format!("Error: {}", e);
                    }
                    _ => {}
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = AppMode::Dashboard;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_changelog_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') || key.code == KeyCode::Char('c') {
            self.mode = AppMode::Dashboard;
        }
        Ok(())
    }

    async fn handle_server_config_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        let mut server_id = String::new();
        let mut field = 0;
        let mut editing = false;
        let mut config = crate::daemon::types::ServerConfig::default();

        match &self.mode {
            AppMode::ServerConfig { server_id: sid, field: f, editing: e, config: c } => {
                server_id = sid.clone();
                field = *f;
                editing = *e;
                config = c.clone();
            }
            _ => return Ok(()),
        };

        if editing {
            match key.code {
                KeyCode::Esc => {
                    self.mode = AppMode::ServerConfig { server_id, field, editing: false, config };
                }
                KeyCode::Enter => {
                    self.mode = AppMode::ServerConfig { server_id, field, editing: false, config };
                }
                KeyCode::Char(c) => {
                    self.update_config_field(&mut config, field, c);
                    self.mode = AppMode::ServerConfig { server_id, field, editing: true, config };
                }
                KeyCode::Backspace => {
                    self.backspace_config_field(&mut config, field);
                    self.mode = AppMode::ServerConfig { server_id, field, editing: true, config };
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Tab => {
                    self.mode = AppMode::ServerConfig { server_id, field: (field + 1) % 9, editing: false, config };
                }
                KeyCode::BackTab => {
                    let new_field = if field == 0 { 8 } else { field - 1 };
                    self.mode = AppMode::ServerConfig { server_id, field: new_field, editing: false, config };
                }
                KeyCode::Enter => {
                    if field == 8 {
                        self.save_server_config(config).await?;
                    } else {
                        self.mode = AppMode::ServerConfig { server_id, field, editing: true, config };
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = AppMode::Dashboard;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn update_config_field(&mut self, config: &mut crate::daemon::types::ServerConfig, field: usize, c: char) {
        match field {
            0 => config.java_path.push(c),
            1 => config.xms.push(c),
            2 => config.xmx.push(c),
            3 => config.jvm_extra_args.push(c),
            4 => { let v = format!("{}{}", config.server_port, c); if let Ok(v) = v.parse() { config.server_port = v; }},
            5 => { let v = format!("{}{}", config.max_players, c); if let Ok(v) = v.parse() { config.max_players = v; }},
            6 => config.online_mode = !config.online_mode,
            7 => config.motd.push(c),
            _ => {}
        }
    }

    fn backspace_config_field(&mut self, config: &mut crate::daemon::types::ServerConfig, field: usize) {
        match field {
            0 => { config.java_path.pop(); }
            1 => { config.xms.pop(); }
            2 => { config.xmx.pop(); }
            3 => { config.jvm_extra_args.pop(); }
            4 => { config.server_port = config.server_port / 10; }
            5 => { config.max_players = config.max_players / 10; }
            6 => { config.online_mode = !config.online_mode; }
            7 => { config.motd.pop(); }
            _ => {}
        }
    }

    async fn save_server_config(&mut self, config: crate::daemon::types::ServerConfig) -> anyhow::Result<()> {
        match self.client.send(IpcRequest::SetServerConfig { config }) {
            Ok(IpcResponse::Result { success, message }) => {
                self.status_message = message;
                if success {
                    self.mode = AppMode::Dashboard;
                    self.refresh_servers().await?;
                }
            }
            Ok(IpcResponse::Error { message }) => {
                self.status_message = format!("Error: {}", message);
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
            }
            _ => {
                self.status_message = "Respuesta inesperada".to_string();
            }
        }
        Ok(())
    }

    async fn execute_server_action(&mut self, action: ServerAction, server_id: &str) -> anyhow::Result<()> {
        let request = match action {
            ServerAction::Start => IpcRequest::StartServer { server_id: server_id.to_string() },
            ServerAction::Stop => IpcRequest::StopServer { server_id: server_id.to_string() },
            ServerAction::Restart => IpcRequest::RestartServer { server_id: server_id.to_string() },
        };

        match self.client.send(request) {
            Ok(IpcResponse::Result { success, message }) => {
                self.status_message = message;
                if success {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    self.refresh_servers().await?;
                }
            }
            Ok(IpcResponse::Error { message }) => {
                self.status_message = format!("Error: {}", message);
            }
            Ok(_) => {
                self.status_message = "Respuesta inesperada".to_string();
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
            }
        }
        Ok(())
    }

    async fn attach_console(&mut self, server_id: String) -> anyhow::Result<()> {
        if let Some(server) = self.servers.iter().find(|s| s.id == server_id) {
            if let Some(pid) = server.pid {
                self.console_session_id = Some(pid);
                self.console_server_id = Some(server.id.clone());
                self.mode = AppMode::Console { server_id };
                self.status_message = format!("Consola adjunta a {} (PID: {}) - Ctrl+\\ para detach", server.name, pid);
            } else {
                self.status_message = "Servidor no esta corriendo".to_string();
            }
        }
        Ok(())
    }

    async fn detach_console(&mut self) -> anyhow::Result<()> {
        self.console_session_id = None;
        self.console_server_id = None;
        self.mode = AppMode::Dashboard;
        self.status_message = "Desconectado de consola. Servidor sigue corriendo.".to_string();
        Ok(())
    }

    fn key_to_command(&self, key: KeyEvent) -> String {
        match key.code {
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => c.to_string(),
            KeyCode::Enter => "\n".to_string(),
            KeyCode::Tab => "\t".to_string(),
            KeyCode::Backspace => "\x08".to_string(),
            KeyCode::Delete => "\x7f".to_string(),
            KeyCode::Esc => "\x1b".to_string(),
            KeyCode::Up => "\x1b[A".to_string(),
            KeyCode::Down => "\x1b[B".to_string(),
            KeyCode::Left => "\x1b[D".to_string(),
            KeyCode::Right => "\x1b[C".to_string(),
            _ => String::new(),
        }
    }

    pub fn add_log(&mut self, line: String) {
        self.logs.push(line);
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    pub fn selected_server(&self) -> Option<&ServerInfo> {
        self.servers.get(self.selected)
    }

    async fn check_updates(&mut self) -> anyhow::Result<()> {
        match self.client.send(IpcRequest::CheckUpdates) {
            Ok(IpcResponse::CheckUpdatesResponse { current, latest, downloaded }) => {
                if current == latest {
                    self.status_message = format!("Actualizado a la version {}", current);
                } else {
                    let changelog = self.fetch_changelog(&latest).await.unwrap_or_else(|_| "No disponible".to_string());
                    self.mode = AppMode::UpdateAvailable { version: latest.clone(), download_url: String::new(), downloaded };
                    self.status_message = format!("Actualizacion disponible: {} -> {} (descargada: {})", current, latest, downloaded);
                }
            }
            Ok(IpcResponse::Error { message }) => {
                self.status_message = format!("Error: {}", message);
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
            }
            _ => {
                self.status_message = "Respuesta inesperada".to_string();
            }
        }
        Ok(())
    }

    async fn fetch_changelog(&self, version: &str) -> anyhow::Result<String> {
        let client = reqwest::blocking::Client::new();
        let url = format!(
            "https://api.github.com/repos/FerNandoGD24/Minecraft-Server-Manager-Rust/releases/tags/v{}",
            version
        );

        #[derive(serde::Deserialize)]
        struct GitHubRelease {
            body: String,
            html_url: String,
        }

        let release: GitHubRelease = client
            .get(&url)
            .header("User-Agent", "mc-manager-updater")
            .send()?
            .json()?;

        Ok(release.body)
    }

    /// Carga config del servidor y abre editor
    async fn load_server_config(&mut self, server_id: String) -> anyhow::Result<()> {
        match self.client.send(IpcRequest::GetServerConfig { server_id: server_id.clone() }) {
            Ok(IpcResponse::ServerConfig { config }) => {
                self.mode = AppMode::ServerConfig {
                    server_id,
                    field: 0,
                    editing: false,
                    config,
                };
                self.status_message = "Editando config: Tab/Shift+Tab navega, Enter edita, Enter en Guardar aplica".to_string();
            }
            Ok(IpcResponse::Error { message }) => {
                self.status_message = format!("Error: {}", message);
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
            }
            _ => {
                self.status_message = "Respuesta inesperada".to_string();
            }
        }
        Ok(())
    }
}