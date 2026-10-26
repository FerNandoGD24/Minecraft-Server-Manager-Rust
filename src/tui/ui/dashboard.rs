use crate::tui::app::App;
use crate::daemon::ServerInfo;
use crate::monitoring::ResourceMetrics;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

/// Renderiza el dashboard principal
pub fn render_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Panel izquierdo: lista de servidores
    render_server_list(f, app, chunks[0]);

    // Panel derecho: métricas + logs
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),  // Métricas
            Constraint::Min(8),      // Logs
        ])
        .split(chunks[1]);

    render_metrics_panel(f, app, right_chunks[0]);
    render_logs_panel(f, app, right_chunks[1]);
}

/// Renderiza lista de servidores seleccionable
fn render_server_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .servers
        .iter()
        .enumerate()
        .map(|(i, server)| {
            let status_icon = if server.status == "running" { "●" } else { "○" };
            let status_color = if server.status == "running" {
                Color::Green
            } else {
                Color::Red
            };

            let style = if i == app.selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(server.name.clone(), style),
                Span::styled(format!(" [{}]", server.service), Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" SERVIDORES ({}) ", app.servers.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.selected));

    f.render_stateful_widget(list, area, &mut state);
}

/// Renderiza panel de métricas
fn render_metrics_panel(f: &mut Frame, app: &App, area: Rect) {
    let server = app.selected_server();

    let metrics_text = if let Some(server) = server {
        // Buscar métricas para este servidor
        let metrics = app.metrics.get(&server.id);

        let cpu = metrics.map(|m| format!("{:.1}%", m.cpu_percent)).unwrap_or_else(|| "N/A".to_string());
        let ram = metrics.map(|m| format!("{} / {} MB", m.memory_rss / 1024 / 1024, m.memory_virtual / 1024 / 1024)).unwrap_or_else(|| "N/A".to_string());
        let disk_r = metrics.map(|m| format!("{} KB/s", m.disk_read_bps / 1024)).unwrap_or_else(|| "N/A".to_string());
        let disk_w = metrics.map(|m| format!("{} KB/s", m.disk_write_bps / 1024)).unwrap_or_else(|| "N/A".to_string());
        let net_rx = metrics.map(|m| format!("{} KB/s", m.net_rx_bps / 1024)).unwrap_or_else(|| "N/A".to_string());
        let net_tx = metrics.map(|m| format!("{} KB/s", m.net_tx_bps / 1024)).unwrap_or_else(|| "N/A".to_string());

        vec![
            Line::from(vec![
                Span::styled("CPU:    ", Style::default().fg(Color::Gray)),
                Span::styled(cpu, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("RAM:    ", Style::default().fg(Color::Gray)),
                Span::styled(ram, Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("DISK R: ", Style::default().fg(Color::Gray)),
                Span::styled(disk_r, Style::default().fg(Color::Blue)),
            ]),
            Line::from(vec![
                Span::styled("DISK W: ", Style::default().fg(Color::Gray)),
                Span::styled(disk_w, Style::default().fg(Color::Blue)),
            ]),
            Line::from(vec![
                Span::styled("NET RX: ", Style::default().fg(Color::Gray)),
                Span::styled(net_rx, Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("NET TX: ", Style::default().fg(Color::Gray)),
                Span::styled(net_tx, Style::default().fg(Color::Magenta)),
            ]),
        ]
    } else {
        vec![Line::from("Selecciona un servidor para ver métricas")]
    };

    let paragraph = Paragraph::new(metrics_text)
        .block(
            Block::default()
                .title(" MÉTRICAS ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Renderiza panel de logs
fn render_logs_panel(f: &mut Frame, app: &App, area: Rect) {
    let logs_text: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|log| Line::from(Span::raw(log.clone())))
        .collect();

    let paragraph = Paragraph::new(logs_text)
        .block(
            Block::default()
                .title(" LOGS (últimas 10) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Renderiza barra de estado inferior
pub fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let daemon_status = if app.daemon_connected {
        Span::styled(" ● DAEMON CONNECTED ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" ○ DAEMON DISCONNECTED ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    };

    let mode_hint = match app.mode {
        crate::tui::app::AppMode::Dashboard => " [↑↓] Select  [Enter] Console  [s] Start  [x] Stop  [r] Restart  [u] Updates  [e] Config  [?] Help  [q] Quit ",
        crate::tui::app::AppMode::Console { .. } => " [Type] Send to console  [Ctrl+\\] Detach ",
        crate::tui::app::AppMode::Help => " [?/Esc/q] Close help ",
        crate::tui::app::AppMode::ConfirmAction { .. } => " [y/Enter] Confirm  [n/Esc] Cancel ",
        crate::tui::app::AppMode::UpdateAvailable { .. } => " [c] Changelog  [d] Download  [Esc/q] Cancel ",
        crate::tui::app::AppMode::Changelog { .. } => " [Esc/q/c] Close changelog ",
        crate::tui::app::AppMode::ServerConfig { editing, .. } => if editing { " ESC/Enter: salir edición" } else { " Tab/Shift+Tab: navegar  |  Enter: editar/guardar  |  ESC/q: cancelar" },
    };

    let status = Span::styled(&app.status_message, Style::default().fg(Color::Yellow));

    let line = Line::from(vec![daemon_status, Span::raw(" | "), status, Span::raw(" | "), Span::styled(mode_hint, Style::default().fg(Color::DarkGray))]);

    let paragraph = Paragraph::new(line).alignment(Alignment::Left);
    f.render_widget(paragraph, area);
}

/// Renderiza overlay de ayuda
pub fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("KEYBINDINGS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled("Dashboard:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from("  ↑/k, ↓/j     - Seleccionar servidor"),
        Line::from("  Enter        - Attach a consola"),
        Line::from("  s            - Iniciar servidor"),
        Line::from("  x            - Detener servidor"),
        Line::from("  r            - Reiniciar servidor"),
        Line::from("  F5           - Refrescar lista"),
        Line::from(""),
        Line::from(vec![Span::styled("Consola:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from("  (escribe)    - Enviar comando al servidor"),
        Line::from("  Ctrl+\\       - Detach (servidor sigue vivo)"),
        Line::from(""),
        Line::from(vec![Span::styled("General:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from("  ?            - Mostrar/ocultar esta ayuda"),
        Line::from("  q / Ctrl+C   - Salir (daemon sigue vivo)"),
        Line::from(""),
        Line::from(vec![Span::styled("ARQUITECTURA:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
        Line::from("  TUI ←→ Daemon (Unix socket JSON)"),
        Line::from("  Consola directa al proceso (sin daemon)"),
        Line::from(""),
        Line::from(vec![Span::styled("Presiona ? o Esc para cerrar", Style::default().fg(Color::DarkGray))]),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" AYUDA ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });

    let popup_area = centered_rect(70, 80, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// Renderiza overlay de confirmación
pub fn render_confirm(f: &mut Frame, app: &App, area: Rect) {
    if let crate::tui::app::AppMode::ConfirmAction { action, server_id } = &app.mode {
        let server_name = app.servers.iter().find(|s| s.id == *server_id).map(|s| s.name.as_str()).unwrap_or("desconocido");

        let action_text = match action {
            crate::tui::app::ServerAction::Start => "INICIAR",
            crate::tui::app::ServerAction::Stop => "DETENER",
            crate::tui::app::ServerAction::Restart => "REINICIAR",
        };

        let text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(format!("¿{} {}?", action_text, server_name), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from("Esta acción afectará al servidor seleccionado."),
            Line::from(""),
            Line::from(vec![Span::styled("[y/Enter] Sí  [n/Esc] No", Style::default().fg(Color::Cyan))]),
        ];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(" CONFIRMACIÓN ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

        let popup_area = centered_rect(50, 30, area);
        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}

/// Renderiza overlay de actualización disponible
pub fn render_update_available(f: &mut Frame, app: &App, area: Rect) {
    if let crate::tui::app::AppMode::UpdateAvailable { version, downloaded, .. } = &app.mode {
        let text = vec![
            Line::from(""),
            Line::from(vec![Span::styled("ACTUALIZACIÓN DISPONIBLE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from(vec![Span::styled(format!("Versión: {}", version), Style::default().fg(Color::Cyan))]),
            Line::from(vec![Span::styled(format!("Descargado: {}", if *downloaded { "Sí" } else { "No" }), Style::default().fg(Color::Green))]),
            Line::from(""),
            Line::from("Acciones:"),
            Line::from("  [c] Ver changelog"),
            Line::from("  [d] Descargar y aplicar ahora"),
            Line::from("  [Esc/q] Cancelar"),
        ];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(" ACTUALIZACIÓN ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

        let popup_area = centered_rect(60, 35, area);
        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}

/// Renderiza overlay de changelog
pub fn render_changelog(f: &mut Frame, app: &App, area: Rect) {
    if let crate::tui::app::AppMode::Changelog { version, content } = &app.mode {
        let lines: Vec<Line> = vec![
            Line::from(vec![Span::styled(format!("CHANGELOG v{}", version), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(""),
        ].into_iter()
        .chain(content.lines().map(|l| Line::from(Span::raw(l))))
        .collect();

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" CHANGELOG ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });

        let popup_area = centered_rect(80, 80, area);
        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}

/// Renderiza overlay de configuración del servidor
pub fn render_server_config(f: &mut Frame, app: &App, area: Rect) {
    if let crate::tui::app::AppMode::ServerConfig { config, field, editing, .. } = &app.mode {
        let fields = [
            ("Java Path", &config.java_path),
            ("RAM Inicial (Xms)", &config.xms),
            ("RAM Máxima (Xmx)", &config.xmx),
            ("JVM Extra Args", &config.jvm_extra_args),
            ("Puerto", &config.server_port.to_string()),
            ("Max Jugadores", &config.max_players.to_string()),
            ("Online Mode", &if config.online_mode { "true (solo premium)".to_string() } else { "false (cracked)".to_string() }),
            ("MOTD", &config.motd),
            ("Guardar", &"".to_string()), // Guardar
        ];

        let lines: Vec<Line> = fields.iter().enumerate().map(|(i, (label, value))| {
            let is_selected = *field == i;
            let is_editing = *editing && is_selected;

            let prefix = if is_selected { "► " } else { "  " };
            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let value_display = if is_editing {
                format!("{}_", value)
            } else {
                value.to_string()
            };

            if i == 8 {
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled("[ Guardar cambios ]", style),
                ])
            } else if i == 6 {
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{}: ", label), Style::default().fg(Color::Gray)),
                    Span::styled(value_display, style),
                    Span::styled("  [Enter para toggle]", Style::default().fg(Color::DarkGray)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{}: ", label), Style::default().fg(Color::Gray)),
                    Span::styled(value_display, style),
                ])
            }
        }).collect();

        let mut all_lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled("CONFIGURACIÓN DEL SERVIDOR", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(""),
        ];
        all_lines.extend(lines);
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(vec![Span::styled(
            if *editing { "ESC/Enter: salir edición  |  Tab/Shift+Tab: navegar" } else { "Tab/Shift+Tab: navegar  |  Enter: editar/guardar  |  ESC/q: cancelar" },
            Style::default().fg(Color::DarkGray)
        )]));

        let paragraph = Paragraph::new(all_lines)
            .block(
                Block::default()
                    .title(" CONFIGURACIÓN ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        let popup_area = centered_rect(70, 70, area);
        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}

/// Helper para centrar un rect
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