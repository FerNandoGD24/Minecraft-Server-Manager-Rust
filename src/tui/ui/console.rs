use crate::tui::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Renderiza la vista de consola interactiva
pub fn render_console(f: &mut Frame, app: &App, area: Rect) {
    if let crate::tui::app::AppMode::Console { server_id } = &app.mode {
        if let Some(server) = app.servers.iter().find(|s| s.id == *server_id) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),   // Header
                    Constraint::Min(10),     // Output
                    Constraint::Length(3),   // Input hint
                ])
                .split(area);

            // Header
            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("CONSOLA: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(&server.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" (PID: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(server.pid.map(|p| p.to_string()).unwrap_or_else(|| "?".to_string()), Style::default().fg(Color::Green)),
                    Span::styled(")", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("Ctrl+\\ para detach • Servidor SIGUE VIVO al desconectar", Style::default().fg(Color::DarkGray)),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            );
            f.render_widget(header, chunks[0]);

            // Output area - mostrar últimas líneas del buffer
            let output_lines: Vec<Line> = app
                .logs
                .iter()
                .rev()
                .take(chunks[1].height as usize - 2)
                .rev()
                .map(|log| Line::from(Span::raw(log.clone())))
                .collect();

            let output = Paragraph::new(output_lines)
                .block(
                    Block::default()
                        .title(" OUTPUT ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green)),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(output, chunks[1]);

            // Input hint
            let hint = Paragraph::new(vec![Line::from(vec![
                Span::styled("Escribe comandos (ej: ", Style::default().fg(Color::DarkGray)),
                Span::styled("say hola", Style::default().fg(Color::Cyan)),
                Span::styled(", ", Style::default().fg(Color::DarkGray)),
                Span::styled("stop", Style::default().fg(Color::Red)),
                Span::styled(", ", Style::default().fg(Color::DarkGray)),
                Span::styled("help", Style::default().fg(Color::Blue)),
                Span::styled(") • Enter para enviar", Style::default().fg(Color::DarkGray)),
            ])])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
            f.render_widget(hint, chunks[2]);
        }
    }
}

/// Renderiza overlay de ayuda específica de consola
pub fn render_console_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("CONSOLA INTERACTIVA", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled("Comandos útiles:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from("  help           - Lista comandos del servidor"),
        Line::from("  say <msg>      - Envía mensaje a jugadores"),
        Line::from("  list           - Lista jugadores conectados"),
        Line::from("  stop           - Detiene el servidor (graceful)"),
        Line::from("  save-all       - Guarda mundo"),
        Line::from("  save-off/on    - Desactiva/activa auto-save"),
        Line::from("  whitelist <on/off/add/remove> - Gestión whitelist"),
        Line::from("  op <player>    - Da OP a jugador"),
        Line::from("  deop <player>  - Quita OP a jugador"),
        Line::from("  kick <player>  - Expulsa jugador"),
        Line::from("  ban <player>   - Banea jugador"),
        Line::from(""),
        Line::from(vec![Span::styled("Navegación:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from("  ↑/↓           - Historial de comandos (si implementado)"),
        Line::from("  Tab           - Autocompletado (si implementado)"),
        Line::from("  Ctrl+C        - Envía SIGINT al servidor (cuidado)"),
        Line::from("  Ctrl+\\        - Detach (VUELVE A DASHBOARD, servidor vivo)"),
        Line::from(""),
        Line::from(vec![Span::styled("IMPORTANTE:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))]),
        Line::from("  Al hacer detach (Ctrl+\\), el servidor SIGUE CORRIENDO."),
        Line::from("  Puedes re-adjuntar luego con Enter en el dashboard."),
        Line::from(""),
        Line::from(vec![Span::styled("Presiona Esc para cerrar", Style::default().fg(Color::DarkGray))]),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" AYUDA CONSOLA ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    let popup_area = centered_rect(80, 80, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}