use crate::daemon::types::{DaemonConfig, IpcRequest, IpcResponse, ServerInfo, ServerConfig};
use crate::discovery::types::ServerStatus;
use crate::control::ServerManager;
use crate::monitoring::ResourceCollector;
use crate::console::ConsoleManager;
use crate::server_config;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Handler principal del daemon que integra todos los modulos
pub struct DaemonHandler {
    pub config: DaemonConfig,
    pub discovery_search_dirs: Vec<PathBuf>,
    pub server_managers: HashMap<String, Arc<Mutex<ServerManager>>>,
    resource_collector: Option<ResourceCollector>,
    console_manager: Option<ConsoleManager>,
}

impl DaemonHandler {
    pub fn new(config: DaemonConfig, search_dirs: Vec<PathBuf>) -> Self {
        DaemonHandler {
            config,
            discovery_search_dirs: search_dirs,
            server_managers: HashMap::new(),
            resource_collector: None,
            console_manager: None,
        }
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        let mut collector = ResourceCollector::start(crate::monitoring::MonitoringConfig::default());
        self.resource_collector = Some(collector);

        let console_config = crate::console::ConsoleConfig::default();
        self.console_manager = Some(ConsoleManager::new(console_config));

        self.refresh_servers()?;

        Ok(())
    }

    pub fn refresh_servers(&mut self) -> Result<(), String> {
        let servers = crate::discovery::scan_servers(&self.discovery_search_dirs);
        let configs = server_config::load_all_server_configs();

        for server in &servers {
            let id = server.id.clone();
            let server_path = server.path.clone();

            if !self.server_managers.contains_key(&id) {
                let manager = ServerManager::new(server.clone());
                self.server_managers.insert(id.clone(), Arc::new(Mutex::new(manager)));
            }

            // Load or create server config and generate files
            if let Some(config) = configs.get(&id) {
                let mut config = config.clone();
                config.server_id = id.clone();
                let server_dir = PathBuf::from(&server_path);
                if let Err(e) = config.generate_all(&server_dir) {
                    eprintln!("Error generando config para {}: {}", id, e);
                }
            } else {
                // Create default config for new server
                let mut config = server_config::ServerConfig::default();
                config.server_id = id.clone();
                let server_dir = PathBuf::from(&server_path);
                if let Err(e) = config.generate_all(&server_dir) {
                    eprintln!("Error generando config por defecto para {}: {}", id, e);
                }
                if let Err(e) = config.save() {
                    eprintln!("Error guardando config por defecto para {}: {}", id, e);
                }
            }
        }

        let pids: Vec<u32> = servers.iter().filter_map(|s| s.pid).collect();
        if let Some(ref mut collector) = self.resource_collector {
            collector.update_pids(pids);
        }

        Ok(())
    }
}
