use crate::config::{load_config as load_global_config, Config};
use crate::discovery::types::DiscoveryConfig;

/// Carga configuracion de discovery desde config global
pub fn load_config() -> DiscoveryConfig {
    let global = load_global_config();
    DiscoveryConfig::from(&global)
}

/// Guarda configuracion (delegado al config global - no implementado individualmente)
pub fn save_config(_config: &DiscoveryConfig) -> bool {
    // La persistencia se maneja a nivel global via Config
    true
}