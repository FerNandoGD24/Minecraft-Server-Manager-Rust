use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListState, Paragraph},
};
use std::io::{self, stdout};

enum Pantalla {
    Menu,
    Servidores,
    Puente,
}

fn area_central(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(7),
            Constraint::Percentage(35),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(32),
            Constraint::Percentage(25),
        ])
        .split(vertical[1])[1]
}

fn panel(frame: &mut Frame, titulo: &str, texto: &str) {
    let area = frame.size();
    let bloque = Block::default().title(titulo).borders(Borders::ALL);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    frame.render_widget(Paragraph::new(texto), interior);
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut salida = stdout();
    execute!(salida, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(salida))?;

    let resultado = ejecutar(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    resultado
}

fn ejecutar(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut pantalla = Pantalla::Menu;
    let mut estado = ListState::default();
    estado.select(Some(0));

    loop {
        terminal.draw(|frame| {
            let area = frame.size();

            match pantalla {
                Pantalla::Menu => {
                    let opciones = List::new(["Gestionar servidores", "Gestionar puente", "Salir"])
                        .block(
                            Block::default()
                                .title(" Menú principal ")
                                .title_bottom(" ↑/↓ navegar · Enter aceptar · q salir ")
                                .borders(Borders::ALL),
                        )
                        .highlight_symbol("❯ ")
                        .highlight_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );

                    frame.render_stateful_widget(opciones, area_central(area), &mut estado);
                }
                Pantalla::Servidores => panel(
                    frame,
                    " Gestionar servidores ",
                    "Aquí irá la gestión de servidores.\n\nPulsa Esc para volver.",
                ),
                Pantalla::Puente => panel(
                    frame,
                    " Gestionar puente ",
                    "Aquí irá la gestión del puente.\n\nPulsa Esc para volver.",
                ),
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match pantalla {
                    Pantalla::Menu => match key.code {
                        KeyCode::Up => {
                            let actual = estado.selected().unwrap_or(0);
                            estado.select(Some(actual.saturating_sub(1)));
                        }
                        KeyCode::Down => {
                            let actual = estado.selected().unwrap_or(0);
                            estado.select(Some((actual + 1) % 3));
                        }
                        KeyCode::Enter => match estado.selected().unwrap_or(0) {
                            0 => pantalla = Pantalla::Servidores,
                            1 => pantalla = Pantalla::Puente,
                            _ => break,
                        },
                        KeyCode::Esc | KeyCode::Char('q') => break,
                        _ => {}
                    },
                    _ => match key.code {
                        KeyCode::Esc => pantalla = Pantalla::Menu,
                        KeyCode::Char('q') => break,
                        _ => {}
                    },
                }
            }
        }
    }

    Ok(())
}
