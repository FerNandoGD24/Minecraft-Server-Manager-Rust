use crate::console::handler::{ConsoleHandler, setup_terminal, restore_terminal, write_output};
use crate::console::types::{ConsoleConfig, ConsoleEvent, ConsoleSession};
use crate::control::types::ControlError;
use crate::control::ServerManager;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

/// Manager para sesiones de consola
pub struct ConsoleManager {
    config: ConsoleConfig,
    sessions: HashMap<u32, ConsoleSession>,
    server_managers: HashMap<u32, Arc<Mutex<ServerManager>>>,
    next_session_id: u32,
    // Channels para comunicacion con handlers
    event_senders: HashMap<u32, Sender<ConsoleEvent>>,
}

impl ConsoleManager {
    pub fn new(config: ConsoleConfig) -> Self {
        ConsoleManager {
            config,
            sessions: HashMap::new(),
            server_managers: HashMap::new(),
            next_session_id: 1,
            event_senders: HashMap::new(),
        }
    }

    /// Registra un ServerManager para un PID
    pub fn register_server(&mut self, pid: u32, manager: Arc<Mutex<ServerManager>>) {
        self.server_managers.insert(pid, manager);
    }

    /// Adjunta una consola a un servidor por PID
    pub fn attach(&mut self, pid: u32) -> Result<ConsoleSession, ControlError> {
        // Verificar que el servidor existe y esta corriendo
        let _manager = self.server_managers.get(&pid)
            .ok_or_else(|| ControlError::ServerNotFound(std::path::PathBuf::from(format!("PID {}", pid))))?;

        let session_id = self.next_session_id;
        self.next_session_id += 1;

        let mut session = ConsoleSession::new(
            session_id,
            pid,
            self.config.detach_signal,
        );

        // Setup terminal
        setup_terminal()?;

        // Crear canal para eventos
        let (tx, rx): (Sender<ConsoleEvent>, Receiver<ConsoleEvent>) = bounded(100);
        self.event_senders.insert(session_id, tx.clone());

        // Crear handler y empezar input loop
        let handler = ConsoleHandler::new(session.buffer.clone(), self.config.detach_signal);
        let input_handle = handler.run_input_loop(tx.clone());

        // Thread para leer output del servidor (simulado - en real leeria del proceso)
        let buffer = session.buffer.clone();
        let output_handle = thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                match event {
                    ConsoleEvent::Output(line) => {
                        write_output(&buffer, &line);
                        // Enviar al proceso servidor
                        // En implementacion real: server_manager.send_command(&line)
                    }
                    ConsoleEvent::Error(line) => {
                        write_output(&buffer, &format!("[ERROR] {}", line));
                    }
                    ConsoleEvent::ProcessEnded(code) => {
                        write_output(&buffer, &format!("[Proceso terminado con codigo {}]", code));
                        break;
                    }
                    ConsoleEvent::Resize(cols, rows) => {
                        write_output(&buffer, &format!("[Resize {}x{}]", cols, rows));
                    }
                    ConsoleEvent::DetachRequested => {
                        break;
                    }
                }
            }
        });

        // Esperar a que termine (detach o proceso acaba)
        let _ = input_handle.join();
        let _ = output_handle.join();

        // Cleanup terminal
        let _ = restore_terminal();

        // Marcar sesion como detached
        session.detach();
        self.sessions.insert(session_id, session.clone());

        // Remover sender
        self.event_senders.remove(&session_id);

        Ok(session)
    }

    /// Desadjunta una sesion (cleanup)
    pub fn detach(&mut self, session_id: u32) -> Result<(), ControlError> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.detach();
            // El proceso servidor sigue vivo
            Ok(())
        } else {
            Err(ControlError::IoError("Sesion no encontrada".to_string()))
        }
    }

    /// Envia comando a la consola
    pub fn send_command(&mut self, session_id: u32, cmd: &str) -> Result<(), ControlError> {
        if let Some(sender) = self.event_senders.get(&session_id) {
            sender.send(ConsoleEvent::Output(cmd.to_string()))
                .map_err(|e| ControlError::CommandWriteFailed(e.to_string()))?;
            Ok(())
        } else if let Some(session) = self.sessions.get(&session_id) {
            // Sesion detached pero servidor vivo - usar ServerManager
            if let Some(manager) = self.server_managers.get(&session.server_pid) {
                manager.lock().unwrap().send_command(cmd)
                    .map_err(|e| ControlError::CommandWriteFailed(e.to_string()))?;
                Ok(())
            } else {
                Err(ControlError::IoError("Servidor no registrado".to_string()))
            }
        } else {
            Err(ControlError::IoError("Sesion no encontrada".to_string()))
        }
    }

    /// Obtiene una sesion por ID
    pub fn get_session(&self, session_id: u32) -> Option<&ConsoleSession> {
        self.sessions.get(&session_id)
    }

    /// Lista todas las sesiones
    pub fn list_sessions(&self) -> Vec<&ConsoleSession> {
        self.sessions.values().collect()
    }
}