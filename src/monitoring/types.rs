use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Métricas de recursos de un proceso
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceMetrics {
    /// Uso de CPU en porcentaje (0.0 - 100.0+)
    pub cpu_percent: f32,
    /// Memoria RSS en bytes
    pub memory_rss: u64,
    /// Memoria virtual en bytes
    pub memory_virtual: u64,
    /// Bytes leídos de disco por segundo
    pub disk_read_bps: u64,
    /// Bytes escritos en disco por segundo
    pub disk_write_bps: u64,
    /// Bytes recibidos de red por segundo
    pub net_rx_bps: u64,
    /// Bytes enviados de red por segundo
    pub net_tx_bps: u64,
    /// Timestamp de última actualización (Unix ms)
    pub timestamp_ms: u64,
}

impl ResourceMetrics {
    pub fn new() -> Self {
        ResourceMetrics::default()
    }

    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(self.timestamp_ms) > max_age_ms
    }
}

/// Configuración del monitor de recursos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Intervalo de polling en milisegundos
    pub poll_interval_ms: u64,
    /// PIDs a monitorear
    pub pids: Vec<u32>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        MonitoringConfig {
            poll_interval_ms: 2000,
            pids: vec![],
        }
    }
}

/// Almacén thread-safe de métricas por PID
pub type MetricsStore = Arc<Mutex<HashMap<u32, ResourceMetrics>>>;