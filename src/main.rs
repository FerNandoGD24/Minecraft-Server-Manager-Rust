use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, stdout};

const CONFIG_FILE: &str = "config.json";

#[derive(Serialize, Deserialize, Clone, Default)]
struct Config {
    bridge: Option<String>,
    servers: Vec<Server>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Server {
    name: String,
    path: String,
    version: Option<String>,
    server_type: Option<String>,
}

#[derive(PartialEq)]
enum Screen {
    Main,
    Servers,
    Bridge,
    AddingServer,
    ServerMenu,
    Setup,
}

#[derive(PartialEq)]
enum SetupPhase {
    Version,
    Type,
}

// Estructura para almacenar versión y sus loaders compatibles
struct VersionInfo {
    version: String,
    loaders: Vec<String>, // "Vanilla", "Forge", "Fabric", "NeoForge"
}

struct App {
    screen: Screen,
    index: usize,
    config: Config,
    input_buffer: String,
    selected_server: Option<usize>,
    error_message: Option<String>,
    setup_phase: SetupPhase,
    setup_type_index: usize,
    version_list: Vec<VersionInfo>,
    filtered_versions: Vec<usize>, // Índices de version_list
    version_scroll_index: usize,
    available_loaders: Vec<String>, // Loaders disponibles para la versión elegida
}

impl App {
    fn new() -> Self {
        let config = fs::read_to_string(CONFIG_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let version_list = Self::get_all_minecraft_versions();
        let filtered_versions: Vec<usize> = (0..version_list.len()).collect();

        Self {
            screen: Screen::Main,
            index: 0,
            config,
            input_buffer: String::new(),
            selected_server: None,
            error_message: None,
            setup_phase: SetupPhase::Version,
            setup_type_index: 0,
            version_list,
            filtered_versions,
            version_scroll_index: 0,
            available_loaders: Vec::new(),
        }
    }

    fn get_all_minecraft_versions() -> Vec<VersionInfo> {
        vec![
            VersionInfo {
                version: "1.9.4".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.10".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.10.1".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.10.2".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.11".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.11.1".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.11.2".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.12".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.12.1".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.12.2".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.13".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.13.1".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.13.2".into(),
                loaders: vec!["Forge".into()],
            },
            VersionInfo {
                version: "1.14".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.14.1".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.14.2".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.14.3".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.14.4".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.15".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.15.1".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.15.2".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.16".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.16.1".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.16.2".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.16.3".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.16.4".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.16.5".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.17".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.17.1".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.18".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.18.1".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.18.2".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.19".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.19.1".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.19.2".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.19.3".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.19.4".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20".into(),
                loaders: vec!["Forge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20.1".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20.2".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20.3".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20.4".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20.5".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.20.6".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.21".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.21.1".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.21.2".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.21.3".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.21.4".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "1.21.5".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "26.1".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "26.2".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
            VersionInfo {
                version: "26.3".into(),
                loaders: vec!["Forge".into(), "NeoForge".into(), "Fabric".into()],
            },
        ]
    }

    fn filter_versions(&mut self) {
        let search = self.input_buffer.to_lowercase();
        if search.is_empty() {
            self.filtered_versions = (0..self.version_list.len()).collect();
        } else {
            self.filtered_versions = self
                .version_list
                .iter()
                .enumerate()
                .filter(|(_, v)| v.version.to_lowercase().contains(&search))
                .map(|(i, _)| i)
                .collect();
        }
        self.version_scroll_index = 0;
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.config) {
            let _ = fs::write(CONFIG_FILE, json);
        }
    }

    fn server_name_exists(&self, name: &str) -> bool {
        self.config.servers.iter().any(|s| s.name == name)
    }

    fn is_server_folder_empty(&self, server_index: usize) -> bool {
        if server_index >= self.config.servers.len() {
            return true;
        }
        let path = &self.config.servers[server_index].path;
        match fs::read_dir(path) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => true,
        }
    }

    fn delete_server(&mut self, server_index: usize) -> Result<(), String> {
        if server_index >= self.config.servers.len() {
            return Err("Indice invalido".to_string());
        }

        let server = &self.config.servers[server_index];
        let path = server.path.clone();

        if std::path::Path::new(&path).exists() {
            if let Err(e) = fs::remove_dir_all(&path) {
                return Err(format!("Error al borrar carpeta: {}", e));
            }
        }

        self.config.servers.remove(server_index);
        self.save();

        Ok(())
    }

    fn get_server_menu_options(&self) -> Vec<String> {
        if let Some(idx) = self.selected_server {
            let mut options = Vec::new();

            if self.is_server_folder_empty(idx) {
                options.push("Setup".to_string());
                options.push("Borrar Servidor".to_string());
                options.push("[<] Volver".to_string());
            } else {
                options.push("Iniciar Servidor".to_string());
                options.push("Detener Servidor".to_string());
                options.push("Setup".to_string());
                options.push("Borrar Servidor".to_string());
                options.push("[<] Volver".to_string());
            }

            options
        } else {
            vec!["[<] Volver".to_string()]
        }
    }

    fn get_navigable_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        for s in &self.config.servers {
            let info = match (&s.version, &s.server_type) {
                (Some(v), Some(t)) => format!("v{} - {}", v, t),
                (Some(v), None) => format!("v{} - sin configurar", v),
                _ => "sin configurar".to_string(),
            };
            items.push(format!("{} [{}] ({})", s.name, info, s.path));
        }
        items.push("[+] Agregar Servidor".to_string());
        items.push("[<] Volver al Menu".to_string());
        items
    }

    fn get_display_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        if self.config.servers.is_empty() {
            items.push("(Sin servidores configurados)".to_string());
        } else {
            for s in &self.config.servers {
                let info = match (&s.version, &s.server_type) {
                    (Some(v), Some(t)) => format!("v{} - {}", v, t),
                    (Some(v), None) => format!("v{} - sin configurar", v),
                    _ => "sin configurar".to_string(),
                };
                items.push(format!("{} [{}] ({})", s.name, info, s.path));
            }
        }
        items.push("[+] Agregar Servidor".to_string());
        items.push("[<] Volver al Menu".to_string());
        items
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new();
    let mut running = true;

    while running {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match app.screen {
                    Screen::Main => match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(2),
                        KeyCode::Enter | KeyCode::Char(' ') => match app.index {
                            0 => {
                                app.screen = Screen::Servers;
                                app.index = 0;
                            }
                            1 => {
                                app.screen = Screen::Bridge;
                                app.index = 0;
                            }
                            2 => running = false,
                            _ => {}
                        },
                        _ => {}
                    },
                    Screen::Servers => {
                        let nav_items = app.get_navigable_items();
                        let total_nav = nav_items.len();
                        match key.code {
                            KeyCode::Up => app.index = app.index.saturating_sub(1),
                            KeyCode::Down => app.index = (app.index + 1).min(total_nav - 1),
                            KeyCode::Enter => {
                                if app.index == app.config.servers.len() {
                                    app.screen = Screen::AddingServer;
                                    app.input_buffer.clear();
                                    app.error_message = None;
                                } else if app.index == total_nav - 1 {
                                    app.screen = Screen::Main;
                                    app.index = 0;
                                } else {
                                    app.selected_server = Some(app.index);
                                    app.screen = Screen::ServerMenu;
                                    app.index = 0;
                                }
                            }
                            KeyCode::Char(' ') => {
                                if !app.config.servers.is_empty()
                                    && app.index < app.config.servers.len()
                                {
                                    if let Err(e) = app.delete_server(app.index) {
                                        eprintln!("Error: {}", e);
                                    }
                                    if app.index >= app.config.servers.len() && app.index > 0 {
                                        app.index -= 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Screen::ServerMenu => {
                        let options = app.get_server_menu_options();
                        let total_options = options.len();
                        match key.code {
                            KeyCode::Up => app.index = app.index.saturating_sub(1),
                            KeyCode::Down => app.index = (app.index + 1).min(total_options - 1),
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if let Some(server_idx) = app.selected_server {
                                    let is_empty = app.is_server_folder_empty(server_idx);

                                    if is_empty {
                                        match app.index {
                                            0 => {
                                                app.screen = Screen::Setup;
                                                app.setup_phase = SetupPhase::Version;
                                                app.input_buffer.clear();
                                                app.setup_type_index = 0;
                                                app.error_message = None;
                                                app.filter_versions();
                                            }
                                            1 => {
                                                if let Err(e) = app.delete_server(server_idx) {
                                                    eprintln!("Error: {}", e);
                                                }
                                                app.screen = Screen::Servers;
                                                app.selected_server = None;
                                                app.index = 0;
                                            }
                                            2 => {
                                                app.screen = Screen::Servers;
                                                app.index = server_idx;
                                                app.selected_server = None;
                                            }
                                            _ => {}
                                        }
                                    } else {
                                        match app.index {
                                            0 => { /* Iniciar - pendiente */ }
                                            1 => { /* Detener - pendiente */ }
                                            2 => {
                                                app.screen = Screen::Setup;
                                                app.setup_phase = SetupPhase::Version;
                                                app.input_buffer.clear();
                                                app.setup_type_index = 0;
                                                app.error_message = None;
                                                app.filter_versions();
                                            }
                                            3 => {
                                                if let Err(e) = app.delete_server(server_idx) {
                                                    eprintln!("Error: {}", e);
                                                }
                                                app.screen = Screen::Servers;
                                                app.selected_server = None;
                                                app.index = 0;
                                            }
                                            4 => {
                                                app.screen = Screen::Servers;
                                                app.index = server_idx;
                                                app.selected_server = None;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Screen::Setup => match app.setup_phase {
                        SetupPhase::Version => match key.code {
                            KeyCode::Char(c) => {
                                if c.is_ascii_digit() || c == '.' {
                                    app.input_buffer.push(c);
                                    app.error_message = None;
                                    app.filter_versions();
                                }
                            }
                            KeyCode::Backspace => {
                                app.input_buffer.pop();
                                app.error_message = None;
                                app.filter_versions();
                            }
                            KeyCode::Up => {
                                app.version_scroll_index =
                                    app.version_scroll_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if !app.filtered_versions.is_empty() {
                                    app.version_scroll_index = (app.version_scroll_index + 1)
                                        .min(app.filtered_versions.len() - 1);
                                }
                            }
                            KeyCode::Enter => {
                                if !app.filtered_versions.is_empty() {
                                    let version_idx =
                                        app.filtered_versions[app.version_scroll_index];
                                    let version_info = &app.version_list[version_idx];

                                    // Guardar versión
                                    if let Some(idx) = app.selected_server {
                                        app.config.servers[idx].version =
                                            Some(version_info.version.clone());
                                        app.save();
                                    }

                                    // Guardar loaders disponibles para esta versión
                                    app.available_loaders = version_info.loaders.clone();

                                    app.input_buffer.clear();
                                    app.error_message = None;
                                    app.setup_phase = SetupPhase::Type;
                                    app.setup_type_index = 0;
                                }
                            }
                            KeyCode::Esc => {
                                app.screen = Screen::ServerMenu;
                                app.index = 0;
                                app.input_buffer.clear();
                                app.error_message = None;
                            }
                            _ => {}
                        },
                        SetupPhase::Type => {
                            let total = app.available_loaders.len();
                            match key.code {
                                KeyCode::Up => {
                                    app.setup_type_index = app.setup_type_index.saturating_sub(1)
                                }
                                KeyCode::Down => {
                                    if total > 0 {
                                        app.setup_type_index =
                                            (app.setup_type_index + 1).min(total - 1)
                                    }
                                }
                                KeyCode::Enter | KeyCode::Char(' ') => {
                                    if total > 0 {
                                        if let Some(idx) = app.selected_server {
                                            let server_type =
                                                app.available_loaders[app.setup_type_index].clone();
                                            app.config.servers[idx].server_type = Some(server_type);
                                            app.save();
                                        }
                                        app.screen = Screen::ServerMenu;
                                        app.index = 0;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.screen = Screen::ServerMenu;
                                    app.index = 0;
                                }
                                _ => {}
                            }
                        }
                    },
                    Screen::Bridge => {
                        let current = app.config.bridge.as_deref().unwrap_or("Ninguno");
                        let items = [
                            format!(
                                "Playit {}",
                                if current == "playit" { "[ACTIVO]" } else { "" }
                            ),
                            format!(
                                "ZeroTier {}",
                                if current == "zerotier" {
                                    "[ACTIVO]"
                                } else {
                                    ""
                                }
                            ),
                            "[<] Volver al Menu".to_string(),
                        ];
                        let total = items.len();
                        match key.code {
                            KeyCode::Up => app.index = app.index.saturating_sub(1),
                            KeyCode::Down => app.index = (app.index + 1).min(total - 1),
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if app.index == 0 {
                                    app.config.bridge = Some("playit".into());
                                    app.save();
                                } else if app.index == 1 {
                                    app.config.bridge = Some("zerotier".into());
                                    app.save();
                                } else if app.index == 2 {
                                    app.screen = Screen::Main;
                                    app.index = 0;
                                }
                            }
                            _ => {}
                        }
                    }
                    Screen::AddingServer => match key.code {
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                            app.error_message = None;
                        }
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                            app.error_message = None;
                        }
                        KeyCode::Enter => {
                            let name = app.input_buffer.trim().to_string();
                            if name.is_empty() {
                                app.error_message =
                                    Some("El nombre no puede estar vacio".to_string());
                            } else if app.server_name_exists(&name) {
                                app.error_message =
                                    Some("Ya existe un servidor con ese nombre".to_string());
                            } else {
                                let path = format!("./servidores/{}", name);
                                if let Err(e) = fs::create_dir_all(&path) {
                                    app.error_message =
                                        Some(format!("Error al crear carpeta: {}", e));
                                } else {
                                    app.config.servers.push(Server {
                                        name,
                                        path,
                                        version: None,
                                        server_type: None,
                                    });
                                    app.save();
                                    app.input_buffer.clear();
                                    app.error_message = None;
                                    app.screen = Screen::Servers;
                                    app.index = app.config.servers.len() - 1;
                                }
                            }
                        }
                        KeyCode::Esc => {
                            app.input_buffer.clear();
                            app.error_message = None;
                            app.screen = Screen::Servers;
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    match app.screen {
        Screen::Main => {
            let items = ["Gestionar Servidores", "Puente", "Salir"];
            let list = List::new(items.iter().map(|i| Line::from(*i)).collect::<Vec<_>>())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Menu Principal (Flechas: navegar, Enter: seleccionar)"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);
        }
        Screen::Servers => {
            let display_items = app.get_display_items();
            let list_items: Vec<Line> = display_items
                .iter()
                .map(|i| Line::from(i.as_str()))
                .collect();
            let list = List::new(list_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Servidores (Enter: opciones, Espacio: borrar)"),
                )
                .highlight_style(Style::default().fg(Color::Cyan))
                .highlight_symbol(">> ");

            let real_index = if app.config.servers.is_empty() {
                app.index + 1
            } else {
                app.index
            };
            let safe_index = real_index.min(display_items.len().saturating_sub(1));
            let mut state = ListState::default().with_selected(Some(safe_index));
            f.render_stateful_widget(list, area, &mut state);
        }
        Screen::ServerMenu => {
            let server_name = if let Some(idx) = app.selected_server {
                &app.config.servers[idx].name
            } else {
                "Desconocido"
            };

            let options = app.get_server_menu_options();
            let list = List::new(
                options
                    .iter()
                    .map(|i| Line::from(i.as_str()))
                    .collect::<Vec<_>>(),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Opciones de: {}", server_name)),
            )
            .highlight_style(Style::default().fg(Color::Magenta))
            .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);
        }
        Screen::Setup => {
            let popup_area = centered_rect(70, 60, area);
            f.render_widget(Clear, popup_area);

            match app.setup_phase {
                SetupPhase::Version => {
                    let layout = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(6), Constraint::Min(0)])
                        .split(popup_area);

                    let input_text = vec![
                        Line::from("Paso 1/2: Selecciona la version"),
                        Line::from("Escribe solo numeros y puntos (ej: 1.20.1)"),
                        Line::from("f=Forge, n=NeoForge, m=Fabric"),
                        Line::from(Span::styled(
                            format!("Version: {}_", app.input_buffer),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )),
                    ];

                    let input_widget = Paragraph::new(input_text).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Ingresar Version"),
                    );

                    f.render_widget(input_widget, layout[0]);

                    // Lista de versiones con sus loaders visibles
                    let list_items: Vec<Line> = app
                        .filtered_versions
                        .iter()
                        .map(|&idx| {
                            let v = &app.version_list[idx];
                            let loaders_str = v
                                .loaders
                                .iter()
                                .map(|l| match l.as_str() {
                                    "Forge" => "f",
                                    "NeoForge" => "n",
                                    "Fabric" => "m",
                                    _ => "?",
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            Line::from(format!("{} ({})", v.version, loaders_str))
                        })
                        .collect();

                    let list = List::new(list_items)
                        .block(Block::default().borders(Borders::ALL).title(format!(
                            "Versiones disponibles ({})",
                            app.filtered_versions.len()
                        )))
                        .highlight_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol(">> ");

                    let mut state =
                        ListState::default().with_selected(Some(app.version_scroll_index));
                    f.render_stateful_widget(list, layout[1], &mut state);
                }
                SetupPhase::Type => {
                    // Obtener la versión elegida para mostrarla en el título
                    let chosen_version = app
                        .selected_server
                        .and_then(|idx| app.config.servers[idx].version.clone())
                        .unwrap_or_else(|| "?".to_string());

                    let list_items: Vec<Line> = app
                        .available_loaders
                        .iter()
                        .map(|l| Line::from(l.as_str()))
                        .collect();

                    let list = List::new(list_items)
                        .block(Block::default().borders(Borders::ALL).title(format!(
                            "Paso 2/2: Tipo para v{} (Enter: confirmar, Esc: cancelar)",
                            chosen_version
                        )))
                        .highlight_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol(">> ");
                    let mut state = ListState::default().with_selected(Some(app.setup_type_index));
                    f.render_stateful_widget(list, popup_area, &mut state);
                }
            }
        }
        Screen::Bridge => {
            let current = app.config.bridge.as_deref().unwrap_or("Ninguno");
            let items = [
                format!(
                    "Playit {}",
                    if current == "playit" { "[ACTIVO]" } else { "" }
                ),
                format!(
                    "ZeroTier {}",
                    if current == "zerotier" {
                        "[ACTIVO]"
                    } else {
                        ""
                    }
                ),
                "[<] Volver al Menu".to_string(),
            ];
            let list = List::new(
                items
                    .iter()
                    .map(|i| Line::from(i.as_str()))
                    .collect::<Vec<_>>(),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Puente (Actual: {})", current)),
            )
            .highlight_style(Style::default().fg(Color::Green))
            .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);
        }
        Screen::AddingServer => {
            let popup_area = centered_rect(60, 30, area);
            f.render_widget(Clear, popup_area);

            let mut text = vec![
                Line::from("Nombre del servidor:"),
                Line::from(""),
                Line::from(Span::styled(
                    format!("> {}_", app.input_buffer),
                    Style::default().fg(Color::White),
                )),
            ];

            if let Some(ref err) = app.error_message {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    format!("Error: {}", err),
                    Style::default().fg(Color::Red),
                )));
            }

            let paragraph = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Agregar Nuevo Servidor (Enter: guardar, Esc: cancelar)"),
                )
                .style(Style::default().fg(Color::White));

            f.render_widget(paragraph, popup_area);
        }
    }
}
