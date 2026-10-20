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
}

#[derive(PartialEq)]
enum Screen {
    Main,
    Servers,
    Bridge,
    AddingServer,
    ServerMenu,
}

struct App {
    screen: Screen,
    index: usize,
    config: Config,
    input_buffer: String,
    selected_server: Option<usize>,
    error_message: Option<String>, // Nuevo: para mostrar errores
}

impl App {
    fn new() -> Self {
        let config = fs::read_to_string(CONFIG_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            screen: Screen::Main,
            index: 0,
            config,
            input_buffer: String::new(),
            selected_server: None,
            error_message: None,
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

    fn get_navigable_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        for s in &self.config.servers {
            items.push(format!("{} ({})", s.name, s.path));
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
                items.push(format!("{} ({})", s.name, s.path));
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
                                    app.config.servers.remove(app.index);
                                    app.save();
                                    if app.index >= app.config.servers.len() && app.index > 0 {
                                        app.index -= 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Screen::ServerMenu => {
                        let total_options = 4;
                        match key.code {
                            KeyCode::Up => app.index = app.index.saturating_sub(1),
                            KeyCode::Down => app.index = (app.index + 1).min(total_options - 1),
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                match app.index {
                                    0 => { /* Iniciar - pendiente */ }
                                    1 => { /* Detener - pendiente */ }
                                    2 => { /* Configurar - pendiente */ }
                                    3 => {
                                        app.screen = Screen::Servers;
                                        app.index = app.selected_server.unwrap_or(0);
                                        app.selected_server = None;
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
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
                            app.error_message = None; // Limpiar error al escribir
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
                                    app.config.servers.push(Server { name, path });
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

            let items = [
                "Iniciar Servidor",
                "Detener Servidor",
                "Configurar",
                "[<] Volver",
            ];
            let list = List::new(items.iter().map(|i| Line::from(*i)).collect::<Vec<_>>())
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

            // Construir el contenido del popup con posible mensaje de error
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
