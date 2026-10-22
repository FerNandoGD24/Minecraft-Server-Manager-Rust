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
    #[serde(default)]
    ram_gb: Option<u32>,
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
    Ram,
    Type,
}

struct Loader {
    name: String,
    short: String,
    color: Color,
    min_version: String,
    max_version: String,
}

struct VersionInfo {
    version: String,
    loaders: Vec<String>,
}

struct App {
    screen: Screen,
    index: usize,
    config: Config,
    input_buffer: String,
    ram_buffer: String,
    selected_server: Option<usize>,
    error_message: Option<String>,
    setup_phase: SetupPhase,
    setup_type_index: usize,
    version_list: Vec<VersionInfo>,
    filtered_versions: Vec<usize>,
    version_scroll_index: usize,
    available_loaders: Vec<String>,
}

impl App {
    fn new() -> Self {
        let config = fs::read_to_string(CONFIG_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let version_list = Self::generate_version_list();
        let filtered_versions: Vec<usize> = (0..version_list.len()).collect();

        Self {
            screen: Screen::Main,
            index: 0,
            config,
            input_buffer: String::new(),
            ram_buffer: String::new(),
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

    fn compare_versions(v1: &str, v2: &str) -> i32 {
        let p1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
        let p2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();
        let max_len = p1.len().max(p2.len());

        for i in 0..max_len {
            let a = p1.get(i).unwrap_or(&0);
            let b = p2.get(i).unwrap_or(&0);
            if a < b {
                return -1;
            } else if a > b {
                return 1;
            }
        }
        0
    }

    fn is_version_in_range(version: &str, min: &str, max: &str) -> bool {
        Self::compare_versions(version, min) >= 0 && Self::compare_versions(version, max) <= 0
    }

    fn get_all_versions() -> Vec<String> {
        let mut versions = Vec::new();
        let version_map = vec![
            ("1.9", vec!["1.9.4"]),
            ("1.10", vec!["1.10", "1.10.1", "1.10.2"]),
            ("1.11", vec!["1.11", "1.11.1", "1.11.2"]),
            ("1.12", vec!["1.12", "1.12.1", "1.12.2"]),
            ("1.13", vec!["1.13", "1.13.1", "1.13.2"]),
            ("1.14", vec!["1.14", "1.14.1", "1.14.2", "1.14.3", "1.14.4"]),
            ("1.15", vec!["1.15", "1.15.1", "1.15.2"]),
            (
                "1.16",
                vec!["1.16", "1.16.1", "1.16.2", "1.16.3", "1.16.4", "1.16.5"],
            ),
            ("1.17", vec!["1.17", "1.17.1"]),
            ("1.18", vec!["1.18", "1.18.1", "1.18.2"]),
            ("1.19", vec!["1.19", "1.19.1", "1.19.2", "1.19.3", "1.19.4"]),
            (
                "1.20",
                vec![
                    "1.20", "1.20.1", "1.20.2", "1.20.3", "1.20.4", "1.20.5", "1.20.6",
                ],
            ),
            (
                "1.21",
                vec!["1.21", "1.21.1", "1.21.2", "1.21.3", "1.21.4", "1.21.5"],
            ),
            ("26", vec!["26.1", "26.2", "26.3"]),
        ];

        for (_, subs) in version_map {
            versions.extend(subs.into_iter().map(String::from));
        }
        versions
    }

    fn get_loaders() -> Vec<Loader> {
        vec![
            Loader {
                name: "Vanilla".into(),
                short: "v".into(),
                color: Color::White,
                min_version: "1.9.4".into(),
                max_version: "26.3".into(),
            },
            Loader {
                name: "Forge".into(),
                short: "f".into(),
                color: Color::Red,
                min_version: "1.9.4".into(),
                max_version: "26.3".into(),
            },
            Loader {
                name: "Fabric".into(),
                short: "m".into(),
                color: Color::LightBlue,
                min_version: "1.14".into(),
                max_version: "26.3".into(),
            },
            Loader {
                name: "NeoForge".into(),
                short: "n".into(),
                color: Color::LightGreen,
                min_version: "1.20.1".into(),
                max_version: "26.3".into(),
            },
        ]
    }

    fn generate_version_list() -> Vec<VersionInfo> {
        let all_versions = Self::get_all_versions();
        let loaders = Self::get_loaders();

        all_versions
            .into_iter()
            .map(|version| {
                let compatible: Vec<String> = loaders
                    .iter()
                    .filter(|l| Self::is_version_in_range(&version, &l.min_version, &l.max_version))
                    .map(|l| l.name.clone())
                    .collect();

                VersionInfo {
                    version,
                    loaders: compatible,
                }
            })
            .collect()
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

    fn parse_ram(&self) -> Result<u32, String> {
        let text = self.ram_buffer.trim();
        if text.is_empty() {
            return Err("Debes indicar la RAM en GB".to_string());
        }
        match text.parse::<u32>() {
            Ok(0) => Err("La RAM debe ser mayor a 0 GB".to_string()),
            Ok(n) => Ok(n),
            Err(_) => Err("Valor invalido. Usa numeros enteros (ej: 4)".to_string()),
        }
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

        let path = self.config.servers[server_index].path.clone();
        if std::path::Path::new(&path).exists() {
            if let Err(e) = fs::remove_dir_all(&path) {
                return Err(format!("Error al borrar carpeta: {}", e));
            }
        }

        self.config.servers.remove(server_index);
        self.save();
        Ok(())
    }

    fn server_info(&self, s: &Server) -> String {
        let mut parts = Vec::new();
        if let Some(v) = &s.version {
            parts.push(format!("v{}", v));
        }
        if let Some(t) = &s.server_type {
            parts.push(t.clone());
        }
        if let Some(r) = s.ram_gb {
            parts.push(format!("{} GB", r));
        }
        if parts.is_empty() {
            "sin configurar".to_string()
        } else {
            parts.join(" - ")
        }
    }

    fn get_server_menu_options(&self) -> Vec<String> {
        if let Some(idx) = self.selected_server {
            if self.is_server_folder_empty(idx) {
                vec![
                    "Setup".to_string(),
                    "Borrar Servidor".to_string(),
                    "[<] Volver".to_string(),
                ]
            } else {
                vec![
                    "Iniciar Servidor".to_string(),
                    "Detener Servidor".to_string(),
                    "Setup".to_string(),
                    "Borrar Servidor".to_string(),
                    "[<] Volver".to_string(),
                ]
            }
        } else {
            vec!["[<] Volver".to_string()]
        }
    }

    fn get_display_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        if self.config.servers.is_empty() {
            items.push("(Sin servidores configurados)".to_string());
        } else {
            for s in &self.config.servers {
                items.push(format!("{} [{}] ({})", s.name, self.server_info(s), s.path));
            }
        }
        items.push("[+] Agregar Servidor".to_string());
        items.push("[<] Volver al Menu".to_string());
        items
    }

    fn selected_version(&self) -> String {
        self.selected_server
            .and_then(|idx| self.config.servers[idx].version.clone())
            .unwrap_or_else(|| "?".to_string())
    }

    fn selected_ram(&self) -> String {
        self.selected_server
            .and_then(|idx| self.config.servers[idx].ram_gb)
            .map(|r| format!("{} GB", r))
            .unwrap_or_else(|| "?".to_string())
    }
}

fn loader_color(loader: &str) -> Color {
    App::get_loaders()
        .iter()
        .find(|l| l.name == loader)
        .map(|l| l.color)
        .unwrap_or(Color::Gray)
}

fn loader_short(loader: &str) -> String {
    App::get_loaders()
        .iter()
        .find(|l| l.name == loader)
        .map(|l| l.short.clone())
        .unwrap_or_else(|| "?".to_string())
}

fn build_version_line(info: &VersionInfo) -> Line<'static> {
    let mut spans = vec![Span::raw(format!("{:<8}", info.version)), Span::raw(" ")];

    for loader in &info.loaders {
        spans.push(Span::styled(
            format!("[{}] ", loader_short(loader)),
            Style::default()
                .fg(loader_color(loader))
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
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
        .split(vertical[1])[1]
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
            if key.kind != KeyEventKind::Press {
                continue;
            }

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
                        _ => running = false,
                    },
                    _ => {}
                },

                Screen::Servers => {
                    let total_nav = app.get_display_items().len();
                    match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(total_nav - 1),
                        KeyCode::Enter => {
                            if app.index == app.config.servers.len() {
                                app.screen = Screen::AddingServer;
                                app.input_buffer.clear();
                                app.error_message = None;
                            } else if app.index + 1 == total_nav {
                                app.screen = Screen::Main;
                                app.index = 0;
                            } else {
                                app.selected_server = Some(app.index);
                                app.screen = Screen::ServerMenu;
                                app.index = 0;
                            }
                        }
                        KeyCode::Char(' ') => {
                            if app.index < app.config.servers.len() {
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
                    let total = app.get_server_menu_options().len();
                    match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(total - 1),
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if let Some(server_idx) = app.selected_server {
                                let is_empty = app.is_server_folder_empty(server_idx);
                                let offset = if is_empty { 0 } else { 2 };

                                match app.index {
                                    i if i == offset => {
                                        app.screen = Screen::Setup;
                                        app.setup_phase = SetupPhase::Version;
                                        app.input_buffer.clear();
                                        app.ram_buffer.clear();
                                        app.setup_type_index = 0;
                                        app.error_message = None;
                                        app.filter_versions();
                                    }
                                    i if i == offset + 1 => {
                                        if let Err(e) = app.delete_server(server_idx) {
                                            eprintln!("Error: {}", e);
                                        }
                                        app.screen = Screen::Servers;
                                        app.selected_server = None;
                                        app.index = 0;
                                    }
                                    _ => {
                                        app.screen = Screen::Servers;
                                        app.index = server_idx;
                                        app.selected_server = None;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                Screen::Setup => match app.setup_phase {
                    SetupPhase::Version => match key.code {
                        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                            app.input_buffer.push(c);
                            app.error_message = None;
                            app.filter_versions();
                        }
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                            app.error_message = None;
                            app.filter_versions();
                        }
                        KeyCode::Up => {
                            app.version_scroll_index = app.version_scroll_index.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            if !app.filtered_versions.is_empty() {
                                app.version_scroll_index = (app.version_scroll_index + 1)
                                    .min(app.filtered_versions.len() - 1);
                            }
                        }
                        KeyCode::Enter => {
                            if !app.filtered_versions.is_empty() {
                                let idx = app.filtered_versions[app.version_scroll_index];
                                let version = app.version_list[idx].version.clone();
                                let loaders = app.version_list[idx].loaders.clone();

                                if let Some(s) = app.selected_server {
                                    app.config.servers[s].version = Some(version);
                                    app.save();
                                }
                                app.available_loaders = loaders;
                                app.input_buffer.clear();
                                app.ram_buffer.clear();
                                app.error_message = None;
                                app.setup_phase = SetupPhase::Ram;
                            }
                        }
                        KeyCode::Esc => {
                            app.screen = Screen::ServerMenu;
                            app.index = 0;
                            app.input_buffer.clear();
                            app.ram_buffer.clear();
                            app.error_message = None;
                        }
                        _ => {}
                    },

                    SetupPhase::Ram => match key.code {
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            app.ram_buffer.push(c);
                            app.error_message = None;
                        }
                        KeyCode::Backspace => {
                            app.ram_buffer.pop();
                            app.error_message = None;
                        }
                        KeyCode::Enter => match app.parse_ram() {
                            Ok(gb) => {
                                if let Some(idx) = app.selected_server {
                                    app.config.servers[idx].ram_gb = Some(gb);
                                    app.save();
                                }
                                app.ram_buffer.clear();
                                app.error_message = None;
                                app.setup_type_index = 0;
                                app.setup_phase = SetupPhase::Type;
                            }
                            Err(e) => app.error_message = Some(e),
                        },
                        KeyCode::Esc => {
                            app.screen = Screen::ServerMenu;
                            app.index = 0;
                            app.ram_buffer.clear();
                            app.error_message = None;
                        }
                        _ => {}
                    },

                    SetupPhase::Type => {
                        let total = app.available_loaders.len();
                        match key.code {
                            KeyCode::Up => {
                                app.setup_type_index = app.setup_type_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if total > 0 {
                                    app.setup_type_index =
                                        (app.setup_type_index + 1).min(total - 1);
                                }
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if total > 0 {
                                    if let Some(idx) = app.selected_server {
                                        let kind =
                                            app.available_loaders[app.setup_type_index].clone();
                                        app.config.servers[idx].server_type = Some(kind);
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
                    let total = 3;
                    match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(total - 1),
                        KeyCode::Enter | KeyCode::Char(' ') => match app.index {
                            0 => {
                                app.config.bridge = Some("playit".into());
                                app.save();
                            }
                            1 => {
                                app.config.bridge = Some("zerotier".into());
                                app.save();
                            }
                            _ => {
                                app.screen = Screen::Main;
                                app.index = 0;
                            }
                        },
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
                            app.error_message = Some("El nombre no puede estar vacio".to_string());
                        } else if app.server_name_exists(&name) {
                            app.error_message =
                                Some("Ya existe un servidor con ese nombre".to_string());
                        } else {
                            let path = format!("./servidores/{}", name);
                            if let Err(e) = fs::create_dir_all(&path) {
                                app.error_message = Some(format!("Error al crear carpeta: {}", e));
                            } else {
                                app.config.servers.push(Server {
                                    name,
                                    path,
                                    version: None,
                                    server_type: None,
                                    ram_gb: None,
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
            let display = app.get_display_items();
            let items: Vec<Line> = display.iter().map(|i| Line::from(i.as_str())).collect();
            let list = List::new(items)
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
            let safe = real_index.min(display.len().saturating_sub(1));
            let mut state = ListState::default().with_selected(Some(safe));
            f.render_stateful_widget(list, area, &mut state);
        }

        Screen::ServerMenu => {
            let name = app
                .selected_server
                .map(|idx| app.config.servers[idx].name.clone())
                .unwrap_or_else(|| "Desconocido".to_string());

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
                    .title(format!("Opciones de: {}", name)),
            )
            .highlight_style(Style::default().fg(Color::Magenta))
            .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);
        }

        Screen::Setup => match app.setup_phase {
            SetupPhase::Version => {
                let popup_area = centered_rect(70, 60, area);
                f.render_widget(Clear, popup_area);

                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(0)])
                    .split(popup_area);

                let loaders = App::get_loaders();
                let mut legend_spans = vec![Span::raw("Loaders: ")];
                for l in &loaders {
                    legend_spans.push(Span::styled(
                        format!("[{}] {} ", l.short, l.name),
                        Style::default().fg(l.color).add_modifier(Modifier::BOLD),
                    ));
                }

                let input_text = vec![
                    Line::from("Paso 1/3: Selecciona la version"),
                    Line::from("Escribe solo numeros y puntos (ej: 1.20.1)"),
                    Line::from(""),
                    Line::from(legend_spans),
                    Line::from(""),
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

                let items: Vec<Line> = app
                    .filtered_versions
                    .iter()
                    .map(|&idx| build_version_line(&app.version_list[idx]))
                    .collect();

                // CORREGIDO: usar app.filtered_versions.len() en lugar de items.len()
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        "Versiones disponibles ({})",
                        app.filtered_versions.len()
                    )))
                    .highlight_style(
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                let mut state = ListState::default().with_selected(Some(app.version_scroll_index));
                f.render_stateful_widget(list, layout[1], &mut state);
            }

            SetupPhase::Ram => {
                let ram_area = centered_rect(60, 50, area);
                f.render_widget(Clear, ram_area);

                let mut text = vec![
                    Line::from(format!(
                        "Paso 2/3: Memoria RAM (version {})",
                        app.selected_version()
                    )),
                    Line::from(""),
                    Line::from("Indica cuantos GB de RAM tendra este servidor."),
                    Line::from("Usa numeros enteros, sin la letra GB (ej: 2, 4, 8)"),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("RAM (GB): {}_", app.ram_buffer),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("Enter: continuar | Esc: cancelar"),
                ];

                if let Some(ref err) = app.error_message {
                    text.push(Line::from(""));
                    text.push(Line::from(Span::styled(
                        format!("Error: {}", err),
                        Style::default().fg(Color::Red),
                    )));
                }

                let paragraph = Paragraph::new(text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Configurar RAM"),
                );
                f.render_widget(paragraph, ram_area);
            }

            SetupPhase::Type => {
                let popup_area = centered_rect(70, 60, area);
                f.render_widget(Clear, popup_area);

                let items: Vec<Line> = app
                    .available_loaders
                    .iter()
                    .map(|l| {
                        Line::from(Span::styled(
                            l.clone(),
                            Style::default()
                                .fg(loader_color(l))
                                .add_modifier(Modifier::BOLD),
                        ))
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        "Paso 3/3: Tipo para v{} | RAM: {} (Enter: confirmar, Esc: cancelar)",
                        app.selected_version(),
                        app.selected_ram()
                    )))
                    .highlight_style(
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                let mut state = ListState::default().with_selected(Some(app.setup_type_index));
                f.render_stateful_widget(list, popup_area, &mut state);
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
