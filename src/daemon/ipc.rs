use crate::daemon::types::{DaemonConfig, IpcRequest, IpcResponse};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Servidor IPC que escucha en Unix socket
pub struct IpcServer {
    listener: UnixListener,
    config: DaemonConfig,
}

impl IpcServer {
    /// Crea y bindea el servidor IPC
    pub fn new(config: &DaemonConfig) -> Result<Self, String> {
        // Remover socket anterior si existe
        if config.socket_file.exists() {
            std::fs::remove_file(&config.socket_file)
                .map_err(|e| format!("Error removiendo socket viejo: {}", e))?;
        }

        let listener = UnixListener::bind(&config.socket_file)
            .map_err(|e| format!("Error bindeando socket: {}", e))?;

        // Set permissions
        let _ = std::fs::set_permissions(&config.socket_file, std::fs::Permissions::from_mode(0o660));

        Ok(IpcServer {
            listener,
            config: config.clone(),
        })
    }

    /// Acepta conexiones y las maneja en threads separados
    pub fn run<F>(&self, handler: F) -> Result<(), String>
    where
        F: FnMut(IpcRequest) -> IpcResponse + Send + 'static,
    {
        let handler = Arc::new(std::sync::Mutex::new(handler));
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handler = handler.clone();
                    thread::spawn(move || {
                        if let Err(e) = handle_client(stream, &mut *handler.lock().unwrap()) {
                            eprintln!("Error en cliente IPC: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error aceptando conexion: {}", e);
                }
            }
        }
        Ok(())
    }
}

/// Cliente IPC para enviar requests
#[derive(Clone)]
pub struct IpcClient {
    config: DaemonConfig,
}

impl IpcClient {
    pub fn new(config: &DaemonConfig) -> Self {
        IpcClient { config: config.clone() }
    }

    /// Envia un request y espera respuesta
    pub fn send(&self, request: IpcRequest) -> Result<IpcResponse, String> {
        let mut stream = UnixStream::connect(&self.config.socket_file)
            .map_err(|e| format!("Error conectando a daemon: {}", e))?;

        // Set timeout
        stream.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("Error setting timeout: {}", e))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("Error setting timeout: {}", e))?;

        // Serializar request
        let request_json = serde_json::to_string(&request)
            .map_err(|e| format!("Error serializando request: {}", e))?;
        
        // Enviar con newline delimiter
        writeln!(stream, "{}", request_json)
            .map_err(|e| format!("Error enviando request: {}", e))?;
        stream.flush()
            .map_err(|e| format!("Error flush: {}", e))?;

        // Leer respuesta
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)
            .map_err(|e| format!("Error leyendo respuesta: {}", e))?;

        // Parsear respuesta
        let response: IpcResponse = serde_json::from_str(response_line.trim())
            .map_err(|e| format!("Error parseando respuesta: {}", e))?;

        Ok(response)
    }
}

/// Maneja un cliente individual
fn handle_client<F>(mut stream: UnixStream, handler: &mut F) -> Result<(), String>
where
    F: FnMut(IpcRequest) -> IpcResponse,
{
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();

    reader.read_line(&mut line)
        .map_err(|e| format!("Error leyendo request: {}", e))?;

    let request: IpcRequest = serde_json::from_str(line.trim())
        .map_err(|e| format!("Error parseando request: {}", e))?;

    let response = handler(request);

    let response_json = serde_json::to_string(&response)
        .map_err(|e| format!("Error serializando response: {}", e))?;

    writeln!(stream, "{}", response_json)
        .map_err(|e| format!("Error enviando response: {}", e))?;
    stream.flush()
        .map_err(|e| format!("Error flush response: {}", e))?;

    Ok(())
}