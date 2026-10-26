use crate::monitoring::types::{MetricsStore, MonitoringConfig, ResourceMetrics};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System, ProcessRefreshKind, ProcessesToUpdate};

/// Collector que corre en background y actualiza métricas cada intervalo
pub struct ResourceCollector {
    store: MetricsStore,
    config: MonitoringConfig,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ResourceCollector {
    /// Inicia el collector en un thread background
    pub fn start(config: MonitoringConfig) -> Self {
        let store: MetricsStore = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let running = Arc::new(AtomicBool::new(true));
        let poll_interval = Duration::from_millis(config.poll_interval_ms);

        let store_clone = store.clone();
        let running_clone = running.clone();
        let pids = config.pids.clone();

        let handle = thread::spawn(move || {
            let mut sys = System::new_all();
            let mut last_disk_io = HashMap::<u32, (u64, u64)>::new();
            let mut last_net_io = HashMap::<u32, (u64, u64)>::new();
            let mut last_check = Instant::now();

            while running_clone.load(Ordering::Relaxed) {
                let start = Instant::now();

                // Refresh system info
                sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::everything());

                let mut metrics_map = store_clone.lock().unwrap();

                for &pid in &pids {
                    let pid = Pid::from_u32(pid);
                    if let Some(process) = sys.process(pid) {
                        let mut metrics = ResourceMetrics::new();

                        // CPU
                        metrics.cpu_percent = process.cpu_usage();

                        // Memory
                        metrics.memory_rss = process.memory();
                        metrics.memory_virtual = process.virtual_memory();

                        // Disk I/O - calculate rate
                        let disk_usage = process.disk_usage();
                        let current_disk = (disk_usage.read_bytes, disk_usage.written_bytes);
                        if let Some((last_read, last_write)) = last_disk_io.get(&pid.as_u32()) {
                            let elapsed = last_check.elapsed().as_secs_f64().max(0.001);
                            metrics.disk_read_bps = ((current_disk.0.saturating_sub(*last_read)) as f64 / elapsed) as u64;
                            metrics.disk_write_bps = ((current_disk.1.saturating_sub(*last_write)) as f64 / elapsed) as u64;
                        }
                        last_disk_io.insert(pid.as_u32(), current_disk);

                        // Network I/O - sysinfo doesn't expose per-process net directly on all platforms
                        // We'll use 0 as placeholder; could be extended with /proc/net/dev parsing on Linux
                        metrics.net_rx_bps = 0;
                        metrics.net_tx_bps = 0;
                        if let Some((_last_rx, _last_tx)) = last_net_io.get(&pid.as_u32()) {
                            let _elapsed = last_check.elapsed().as_secs_f64().max(0.001);
                            metrics.net_rx_bps = 0; // placeholder
                            metrics.net_tx_bps = 0; // placeholder
                        }
                        last_net_io.insert(pid.as_u32(), (0, 0));

                        // Timestamp
                        metrics.timestamp_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        metrics_map.insert(pid.as_u32(), metrics);
                    }
                }

                last_check = Instant::now();

                // Sleep for remaining interval
                let elapsed = start.elapsed();
                if elapsed < poll_interval {
                    thread::sleep(poll_interval - elapsed);
                }
            }
        });

        ResourceCollector {
            store,
            config,
            running,
            handle: Some(handle),
        }
    }

    /// Obtiene métricas actuales para un PID
    pub fn get_metrics(&self, pid: u32) -> Option<ResourceMetrics> {
        let store = self.store.lock().ok()?;
        store.get(&pid).cloned()
    }

    /// Obtiene todas las métricas actuales
    pub fn get_all_metrics(&self) -> std::collections::HashMap<u32, ResourceMetrics> {
        self.store.lock().map(|m| m.clone()).unwrap_or_default()
    }

    /// Actualiza la lista de PIDs a monitorear
    pub fn update_pids(&mut self, pids: Vec<u32>) {
        self.config.pids = pids;
    }

    /// Detiene el collector y limpia el thread
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ResourceCollector {
    fn drop(&mut self) {
        self.stop();
    }
}