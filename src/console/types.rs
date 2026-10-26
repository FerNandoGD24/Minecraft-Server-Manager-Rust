use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

/// Senal para detach de la consola
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachSignal {
    /// Ctrl+\ (default)
    CtrlBackslash,
    /// F12
    F12,
    /// Ctrl+D
    CtrlD,
}

impl Default for DetachSignal {
    fn default() -> Self {
        DetachSignal::CtrlBackslash
    }
}

/// Evento de la consola
#[derive(Debug, Clone)]
pub enum ConsoleEvent {
    /// Linea de output del servidor
    Output(String),
    /// Error del servidor
    Error(String),
    /// Servidor termino
    ProcessEnded(i32),
    /// Resize de terminal
    Resize(u16, u16),
    /// Detach solicitado
    DetachRequested,
}

/// Buffer circular para historial de consola
#[derive(Debug)]
pub struct ConsoleBuffer {
    lines: VecDeque<String>,
    max_lines: usize,
}

impl ConsoleBuffer {
    pub fn new(max_lines: usize) -> Self {
        ConsoleBuffer {
            lines: VecDeque::with_capacity(max_lines),
            max_lines,
        }
    }

    pub fn push(&mut self, line: String) {
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn get_last(&self, n: usize) -> Vec<String> {
        self.lines.iter().rev().take(n).rev().cloned().collect()
    }

    pub fn get_all(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }
}

/// Sesion de consola activa
#[derive(Debug, Clone)]
pub struct ConsoleSession {
    pub id: u32,
    pub server_pid: u32,
    pub buffer: Arc<Mutex<ConsoleBuffer>>,
    pub detach_signal: DetachSignal,
    pub attached: bool,
}

impl ConsoleSession {
    pub fn new(id: u32, server_pid: u32, detach_signal: DetachSignal) -> Self {
        ConsoleSession {
            id,
            server_pid,
            buffer: Arc::new(Mutex::new(ConsoleBuffer::new(1000))),
            detach_signal,
            attached: true,
        }
    }

    pub fn add_output(&self, line: String) {
        self.buffer.lock().unwrap().push(line);
    }

    pub fn get_history(&self, lines: usize) -> Vec<String> {
        self.buffer.lock().unwrap().get_last(lines)
    }

    pub fn detach(&mut self) {
        self.attached = false;
    }
}

/// Configuracion de la consola
#[derive(Debug, Clone)]
pub struct ConsoleConfig {
    pub detach_signal: DetachSignal,
    pub history_lines: usize,
    pub show_timestamp: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        ConsoleConfig {
            detach_signal: DetachSignal::default(),
            history_lines: 50,
            show_timestamp: false,
        }
    }
}