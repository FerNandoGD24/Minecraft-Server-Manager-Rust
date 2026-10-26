use crate::console::types::{ConsoleBuffer, ConsoleEvent, DetachSignal};
use crate::control::types::ControlError;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Manejador de la consola en raw mode
pub struct ConsoleHandler {
    buffer: Arc<Mutex<ConsoleBuffer>>,
    detach_signal: DetachSignal,
    running: Arc<parking_lot::Mutex<bool>>,
}

impl ConsoleHandler {
    pub fn new(buffer: Arc<Mutex<ConsoleBuffer>>, detach_signal: DetachSignal) -> Self {
        ConsoleHandler {
            buffer,
            detach_signal,
            running: Arc::new(parking_lot::Mutex::new(true)),
        }
    }

    /// Inicia el loop de lectura de input del usuario
    pub fn run_input_loop(&self, sender: crossbeam_channel::Sender<ConsoleEvent>) -> thread::JoinHandle<()> {
        let running = self.running.clone();
        let detach_signal = self.detach_signal;

        thread::spawn(move || {
            while *running.lock() {
                if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key)) => {
                            if Self::is_detach_key(key, detach_signal) {
                                let _ = sender.send(ConsoleEvent::DetachRequested);
                                break;
                            }
                            // Enviar tecla como comando al servidor
                            let cmd = Self::key_to_command(key);
                            if !cmd.is_empty() {
                                let _ = sender.send(ConsoleEvent::Output(cmd));
                            }
                        }
                        Ok(Event::Resize(cols, rows)) => {
                            let _ = sender.send(ConsoleEvent::Resize(cols, rows));
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
        })
    }

    /// Convierte KeyEvent a string de comando
    fn key_to_command(key: KeyEvent) -> String {
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

    /// Verifica si la tecla coincide con la senal de detach
    fn is_detach_key(key: KeyEvent, signal: DetachSignal) -> bool {
        match signal {
            DetachSignal::CtrlBackslash => {
                key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::CONTROL
            }
            DetachSignal::F12 => key.code == KeyCode::F(12),
            DetachSignal::CtrlD => {
                key.code == KeyCode::Char('d') && key.modifiers == KeyModifiers::CONTROL
            }
        }
    }

    /// Detiene el input loop
    pub fn stop(&self) {
        *self.running.lock() = false;
    }
}

/// Configura terminal para raw mode
pub fn setup_terminal() -> Result<(), ControlError> {
    enable_raw_mode().map_err(|e| ControlError::IoError(e.to_string()))?;
    execute!(io::stdout(), EnterAlternateScreen).map_err(|e| ControlError::IoError(e.to_string()))?;
    Ok(())
}

/// Restaura terminal
pub fn restore_terminal() -> Result<(), ControlError> {
    disable_raw_mode().map_err(|e| ControlError::IoError(e.to_string()))?;
    execute!(io::stdout(), LeaveAlternateScreen).map_err(|e| ControlError::IoError(e.to_string()))?;
    Ok(())
}

/// Escribe output al buffer y stdout
pub fn write_output(buffer: &Arc<Mutex<ConsoleBuffer>>, line: &str) {
    buffer.lock().unwrap().push(line.to_string());
    let _ = writeln!(io::stdout(), "{}", line);
    let _ = io::stdout().flush();
}