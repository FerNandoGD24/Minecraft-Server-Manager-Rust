pub mod app;
pub mod ui;

pub use app::App;
use ui::{dashboard, console};
use crate::tui::app::AppMode;
use crossterm::event::{Event, KeyEvent};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;

/// Ejecuta el loop principal de la TUI
pub async fn run_tui(mut app: App) -> anyhow::Result<()> {
    // Setup terminal
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;

    // Channel para eventos de teclado
    let (tx, mut rx) = mpsc::channel::<Event>(32);

    // Task para leer eventos de teclado
    task::spawn(async move {
        loop {
            if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(event) = crossterm::event::read() {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Main loop
    let tick_rate = Duration::from_millis(50); // 20 FPS
    let mut last_tick = std::time::Instant::now();

    loop {
        // Render
        terminal.draw(|f| {
            let size = f.area();

            // Layout principal: dashboard + status bar
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(10),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(size);

            match app.mode {
                app::AppMode::Dashboard => {
                    dashboard::render_dashboard(f, &mut app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                }
                app::AppMode::Console { .. } => {
                    console::render_console(f, &app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                }
                app::AppMode::Help => {
                    dashboard::render_dashboard(f, &mut app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                    dashboard::render_help(f, size);
                }
                app::AppMode::ConfirmAction { .. } => {
                    dashboard::render_dashboard(f, &mut app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                    dashboard::render_confirm(f, &app, size);
                }
                app::AppMode::UpdateAvailable { .. } => {
                    dashboard::render_dashboard(f, &mut app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                    dashboard::render_update_available(f, &app, size);
                }
                app::AppMode::Changelog { .. } => {
                    dashboard::render_dashboard(f, &mut app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                    dashboard::render_changelog(f, &app, size);
                }
                app::AppMode::ServerConfig { .. } => {
                    dashboard::render_dashboard(f, &mut app, chunks[0]);
                    dashboard::render_status_bar(f, &app, chunks[1]);
                    dashboard::render_server_config(f, &app, size);
                }
            }
        })?;

        // Handle events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if timeout > Duration::ZERO {
            tokio::select! {
                Some(event) = rx.recv() => {
                    match event {
                        Event::Key(key) => {
                            if let Err(e) = app.handle_key(key).await {
                                app.status_message = format!("Error: {}", e);
                            }
                        }
                        Event::Resize(_, _) => {
                            // Terminal resized, will re-render on next tick
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(timeout) => {}
            }
        } else {
            // Poll events immediately if tick overdue
            while let Ok(event) = rx.try_recv() {
                match event {
                    Event::Key(key) => {
                        if let Err(e) = app.handle_key(key).await {
                            app.status_message = format!("Error: {}", e);
                        }
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        }

        last_tick = std::time::Instant::now();

        if app.should_quit {
            break;
        }
    }

    // Cleanup terminal
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::terminal::disable_raw_mode()?;
    terminal.show_cursor()?;

    Ok(())
}