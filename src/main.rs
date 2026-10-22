use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Write, stdout};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const CONFIG_FILE: &str = "config.json";

#[derive(Serialize, Deserialize, Clone, Default)]
struct Config {
    bridge: Option<String>,
    servers: Vec<Server>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Server {
    name: String,
    path: String,
    version: Option<String>,
    server_type: Option<String>,
    #[serde(default)]
    ram_gb: Option<u32>,
}

#[derive(PartialEq)]
enum Screen {
    Main,
    Servers,
    Bridge,
    AddingServer,
    ServerMenu,
    Setup,
    Downloading,
}

#[derive(PartialEq)]
enum SetupPhase {
    Version,
    Ram,
    Type,
}

struct Loader {
    name: String,
    short: String,
    color: Color,
    min_version: String,
    max_version: String,
}

struct VersionInfo {
    version: String,
    loaders: Vec<String>,
    version_type: String,
}

struct App {
    screen: Screen,
    index: usize,
    config: Config,
    input_buffer: String,
    ram_buffer: String,
    selected_server: Option<usize>,
    error_message: Option<String>,
    setup_phase: SetupPhase,
    setup_type_index: usize,
    version_list: Vec<VersionInfo>,
    filtered_versions: Vec<usize>,
    version_scroll_index: usize,
    available_loaders: Vec<String>,
    running_servers: std::collections::HashMap<String, Child>,
    download_status: String,
    download_progress: f32,
    download_error: Option<String>,
    download_done: bool,
    http_client: reqwest::blocking::Client,
}

impl App {
    fn new() -> Self {
        let config = fs::read_to_string(CONFIG_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let http_client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        let version_list = Self::load_versions(&http_client);
        let filtered_versions: Vec<usize> = (0..version_list.len()).collect();

        Self {
            screen: Screen::Main,
            index: 0,
            config,
            input_buffer: String::new(),
            ram_buffer: String::new(),
            selected_server: None,
            error_message: None,
            setup_phase: SetupPhase::Version,
            setup_type_index: 0,
            version_list,
            filtered_versions,
            version_scroll_index: 0,
            available_loaders: Vec::new(),
            running_servers: std::collections::HashMap::new(),
            download_status: String::new(),
            download_progress: 0.0,
            download_error: None,
            download_done: false,
            http_client,
        }
    }

    fn load_versions(client: &reqwest::blocking::Client) -> Vec<VersionInfo> {
        if let Ok(response) = client
            .get("https://launchermeta.mojang.net/mc/game/version_manifest_v2.json")
            .send()
        {
            if let Ok(manifest) = response.json::<serde_json::Value>() {
                if let Some(versions) = manifest["versions"].as_array() {
                    let loaders = Self::get_loaders();
                    let mut result: Vec<VersionInfo> = versions
                        .iter()
                        .filter_map(|v| {
                            let id = v["id"].as_str()?.to_string();
                            let vtype = v["type"].as_str().unwrap_or("release").to_string();
                            let compatible: Vec<String> = loaders
                                .iter()
                                .filter(|l| {
                                    Self::is_version_in_range(&id, &l.min_version, &l.max_version)
                                })
                                .map(|l| l.name.clone())
                                .collect();
                            if compatible.is_empty() {
                                None
                            } else {
                                Some(VersionInfo {
                                    version: id,
                                    loaders: compatible,
                                    version_type: vtype,
                                })
                            }
                        })
                        .collect();
                    result.sort_by(
                        |a, b| match Self::compare_versions(&b.version, &a.version) {
                            -1 => std::cmp::Ordering::Less,
                            1 => std::cmp::Ordering::Greater,
                            _ => std::cmp::Ordering::Equal,
                        },
                    );
                    if !result.is_empty() {
                        return result;
                    }
                }
            }
        }
        Self::generate_version_list_fallback()
    }

    fn generate_version_list_fallback() -> Vec<VersionInfo> {
        let loaders = Self::get_loaders();
        let versions = vec![
            "1.21.5", "1.21.4", "1.21.3", "1.21.2", "1.21.1", "1.21", "1.20.6", "1.20.5", "1.20.4",
            "1.20.3", "1.20.2", "1.20.1", "1.20", "1.19.4", "1.19.3", "1.19.2", "1.19.1", "1.19",
            "1.18.2", "1.18.1", "1.18", "1.17.1", "1.17", "1.16.5", "1.16.4", "1.16.3", "1.16.2",
            "1.16.1", "1.16", "1.15.2", "1.15.1", "1.15", "1.14.4", "1.14.3", "1.14.2", "1.14.1",
            "1.14", "1.13.2", "1.13.1", "1.13", "1.12.2", "1.12.1", "1.12", "1.11.2", "1.11.1",
            "1.11", "1.10.2", "1.10.1", "1.10", "1.9.4", "1.9.2", "1.9.1", "1.9", "1.8.9", "1.8.8",
            "1.8", "1.7.10", "1.7.2",
        ];
        versions
            .into_iter()
            .map(|v| {
                let compatible: Vec<String> = loaders
                    .iter()
                    .filter(|l| Self::is_version_in_range(v, &l.min_version, &l.max_version))
                    .map(|l| l.name.clone())
                    .collect();
                VersionInfo {
                    version: v.to_string(),
                    loaders: compatible,
                    version_type: "release".to_string(),
                }
            })
            .collect()
    }

    fn compare_versions(v1: &str, v2: &str) -> i32 {
        let p1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
        let p2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();
        let max_len = p1.len().max(p2.len());
        for i in 0..max_len {
            let a = p1.get(i).unwrap_or(&0);
            let b = p2.get(i).unwrap_or(&0);
            if a < b {
                return -1;
            } else if a > b {
                return 1;
            }
        }
        0
    }

    fn is_version_in_range(version: &str, min: &str, max: &str) -> bool {
        Self::compare_versions(version, min) >= 0 && Self::compare_versions(version, max) <= 0
    }

    fn get_loaders() -> Vec<Loader> {
        vec![
            Loader {
                name: "Vanilla".into(),
                short: "v".into(),
                color: Color::White,
                min_version: "1.0".into(),
                max_version: "99.0".into(),
            },
            Loader {
                name: "Forge".into(),
                short: "f".into(),
                color: Color::Red,
                min_version: "1.7.2".into(),
                max_version: "99.0".into(),
            },
            Loader {
                name: "Fabric".into(),
                short: "m".into(),
                color: Color::LightBlue,
                min_version: "1.14".into(),
                max_version: "99.0".into(),
            },
            Loader {
                name: "NeoForge".into(),
                short: "n".into(),
                color: Color::LightGreen,
                min_version: "1.20.1".into(),
                max_version: "99.0".into(),
            },
        ]
    }

    fn filter_versions(&mut self) {
        let search = self.input_buffer.to_lowercase();
        if search.is_empty() {
            self.filtered_versions = (0..self.version_list.len()).collect();
        } else {
            self.filtered_versions = self
                .version_list
                .iter()
                .enumerate()
                .filter(|(_, v)| v.version.to_lowercase().contains(&search))
                .map(|(i, _)| i)
                .collect();
        }
        self.version_scroll_index = 0;
    }

    fn parse_ram(&self) -> Result<u32, String> {
        let text = self.ram_buffer.trim();
        if text.is_empty() {
            return Err("Debes indicar la RAM en GB".to_string());
        }
        match text.parse::<u32>() {
            Ok(0) => Err("La RAM debe ser mayor a 0 GB".to_string()),
            Ok(n) => Ok(n),
            Err(_) => Err("Valor invalido. Usa numeros enteros (ej: 4)".to_string()),
        }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.config) {
            let _ = fs::write(CONFIG_FILE, json);
        }
    }

    fn server_name_exists(&self, name: &str) -> bool {
        self.config.servers.iter().any(|s| s.name == name)
    }

    fn is_server_configured(&self, server_index: usize) -> bool {
        if server_index >= self.config.servers.len() {
            return false;
        }
        let s = &self.config.servers[server_index];
        s.version.is_some() && s.server_type.is_some() && s.ram_gb.is_some()
    }

    fn is_server_running(&self, server_index: usize) -> bool {
        if server_index >= self.config.servers.len() {
            return false;
        }
        self.running_servers
            .contains_key(&self.config.servers[server_index].name)
    }

    fn start_server(&mut self, server_index: usize) -> Result<(), String> {
        if server_index >= self.config.servers.len() {
            return Err("Indice invalido".to_string());
        }
        let server = &self.config.servers[server_index];
        if self.running_servers.contains_key(&server.name) {
            return Err("El servidor ya esta en ejecucion".to_string());
        }

        let path = Path::new(&server.path);
        let has_run_sh = path.join("run.sh").exists();
        let has_jar = path.join("server.jar").exists();

        if !has_jar && !has_run_sh {
            return Err(format!(
                "No se encontro server.jar ni run.sh en {}",
                server.path
            ));
        }

        let ram = server.ram_gb.unwrap_or(2);
        let ram_mb = ram * 1024;

        let child = if has_run_sh {
            Command::new("bash")
                .arg("run.sh")
                .current_dir(&server.path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        } else {
            Command::new("java")
                .arg(format!("-Xmx{}M", ram_mb))
                .arg(format!("-Xms{}M", ram_mb))
                .arg("-jar")
                .arg("server.jar")
                .arg("nogui")
                .current_dir(&server.path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        };

        match child {
            Ok(c) => {
                self.running_servers.insert(server.name.clone(), c);
                Ok(())
            }
            Err(e) => Err(format!("Error al iniciar: {}", e)),
        }
    }

    fn stop_server(&mut self, server_index: usize) -> Result<(), String> {
        if server_index >= self.config.servers.len() {
            return Err("Indice invalido".to_string());
        }
        let server = &self.config.servers[server_index];
        if let Some(mut child) = self.running_servers.remove(&server.name) {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "stop");
            }
            let _ = child.wait();
            Ok(())
        } else {
            Err("El servidor no esta en ejecucion".to_string())
        }
    }

    fn delete_server(&mut self, server_index: usize) -> Result<(), String> {
        if server_index >= self.config.servers.len() {
            return Err("Indice invalido".to_string());
        }
        let name = self.config.servers[server_index].name.clone();
        if let Some(mut child) = self.running_servers.remove(&name) {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "stop");
            }
            let _ = child.wait();
        }
        let path = self.config.servers[server_index].path.clone();
        if Path::new(&path).exists() {
            if let Err(e) = fs::remove_dir_all(&path) {
                return Err(format!("Error al borrar carpeta: {}", e));
            }
        }
        self.config.servers.remove(server_index);
        self.save();
        Ok(())
    }

    fn server_info(&self, s: &Server) -> String {
        let mut parts = Vec::new();
        if let Some(v) = &s.version {
            parts.push(format!("v{}", v));
        }
        if let Some(t) = &s.server_type {
            parts.push(t.clone());
        }
        if let Some(r) = s.ram_gb {
            parts.push(format!("{} GB", r));
        }
        if parts.is_empty() {
            "sin configurar".to_string()
        } else {
            parts.join(" - ")
        }
    }

    fn get_server_menu_options(&self) -> Vec<String> {
        if let Some(idx) = self.selected_server {
            if !self.is_server_configured(idx) {
                vec![
                    "Setup".into(),
                    "Borrar Servidor".into(),
                    "[<] Volver".into(),
                ]
            } else if self.is_server_running(idx) {
                vec![
                    "Detener Servidor".into(),
                    "Borrar Servidor".into(),
                    "[<] Volver".into(),
                ]
            } else {
                vec![
                    "Iniciar Servidor".into(),
                    "Borrar Servidor".into(),
                    "[<] Volver".into(),
                ]
            }
        } else {
            vec!["[<] Volver".into()]
        }
    }

    fn get_display_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        if self.config.servers.is_empty() {
            items.push("(Sin servidores configurados)".to_string());
        } else {
            for (i, s) in self.config.servers.iter().enumerate() {
                let status = if self.is_server_running(i) {
                    "[EN LINEA]"
                } else {
                    ""
                };
                items.push(format!(
                    "{} {} [{}] ({})",
                    s.name,
                    status,
                    self.server_info(s),
                    s.path
                ));
            }
        }
        items.push("[+] Agregar Servidor".to_string());
        items.push("[<] Volver al Menu".to_string());
        items
    }

    fn selected_version(&self) -> String {
        self.selected_server
            .and_then(|idx| self.config.servers[idx].version.clone())
            .unwrap_or_else(|| "?".into())
    }

    fn selected_ram(&self) -> String {
        self.selected_server
            .and_then(|idx| self.config.servers[idx].ram_gb)
            .map(|r| format!("{} GB", r))
            .unwrap_or_else(|| "?".into())
    }

    fn download_server(&mut self) {
        let server_idx = match self.selected_server {
            Some(i) => i,
            None => {
                self.download_error = Some("No hay servidor seleccionado".into());
                return;
            }
        };

        let server = self.config.servers[server_idx].clone();
        let version = match server.version {
            Some(v) => v,
            None => {
                self.download_error = Some("Version no definida".into());
                return;
            }
        };
        let loader = match server.server_type {
            Some(l) => l,
            None => {
                self.download_error = Some("Tipo no definido".into());
                return;
            }
        };

        self.download_status = format!("Preparando descarga de {} ({})...", loader, version);
        self.download_progress = 0.0;

        let result = match loader.as_str() {
            "Vanilla" => self.download_vanilla(&server.path, &version),
            "Fabric" => self.download_fabric(&server.path, &version),
            "Forge" => self.download_forge(&server.path, &version),
            "NeoForge" => self.download_neoforge(&server.path, &version),
            _ => Err(format!("Loader {} no soportado", loader)),
        };

        match result {
            Ok(_) => {
                self.download_status = "Descarga completada!".into();
                self.download_progress = 1.0;
                self.download_done = true;
            }
            Err(e) => {
                self.download_error = Some(e);
            }
        }
    }

    fn download_file(&self, url: &str, dest: &Path) -> Result<(), String> {
        let response = self
            .http_client
            .get(url)
            .send()
            .map_err(|e| format!("Error de red: {}\nURL: {}", e, url))?;

        if !response.status().is_success() {
            return Err(format!(
                "Error HTTP {}: {}\nURL: {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown"),
                url
            ));
        }

        let mut file = File::create(dest)
            .map_err(|e| format!("No se pudo crear {}: {}", dest.display(), e))?;
        let bytes = response
            .bytes()
            .map_err(|e| format!("Error leyendo respuesta: {}", e))?;

        file.write_all(&bytes)
            .map_err(|e| format!("Error escribiendo en disco: {}", e))?;

        Ok(())
    }

    fn download_vanilla(&mut self, path: &str, version: &str) -> Result<(), String> {
        let _ = self
            .version_list
            .iter()
            .find(|v| v.version == version)
            .ok_or_else(|| format!("La version {} no esta en la lista de Mojang", version))?;

        self.download_status = format!("Obteniendo detalles de {}...", version);
        self.download_progress = 0.2;

        let manifest_url = "https://launchermeta.mojang.net/mc/game/version_manifest_v2.json";
        let manifest: serde_json::Value = self
            .http_client
            .get(manifest_url)
            .send()
            .map_err(|e| format!("Error conectando a Mojang: {}", e))?
            .json()
            .map_err(|e| format!("Error parseando manifest: {}", e))?;

        let version_entry = manifest["versions"]
            .as_array()
            .ok_or_else(|| "Manifest sin versiones".to_string())?
            .iter()
            .find(|v| v["id"].as_str() == Some(version))
            .ok_or_else(|| format!("Version {} no encontrada en Mojang", version))?;

        let version_url = version_entry["url"]
            .as_str()
            .ok_or_else(|| "URL de detalles no encontrada".to_string())?;

        let version_json: serde_json::Value = self
            .http_client
            .get(version_url)
            .send()
            .map_err(|e| format!("Error obteniendo detalles: {}", e))?
            .json()
            .map_err(|e| format!("Error parseando detalles: {}", e))?;

        let downloads = version_json["downloads"]
            .as_object()
            .ok_or_else(|| "JSON sin seccion 'downloads'".to_string())?;

        let server_download = downloads.get("server").ok_or_else(|| {
            format!(
                "La version {} no tiene server.jar oficial.\n\
                 Tipos disponibles: {:?}",
                version,
                downloads.keys().collect::<Vec<_>>()
            )
        })?;

        let jar_url = server_download["url"]
            .as_str()
            .ok_or_else(|| "URL de server.jar no encontrada".to_string())?;

        let jar_size = server_download["size"].as_u64().unwrap_or(0);

        self.download_status = format!("Descargando server.jar ({} bytes)...", jar_size);
        self.download_progress = 0.5;

        let dest = Path::new(path).join("server.jar");
        self.download_file(jar_url, &dest)?;

        let file_size = dest.metadata().map(|m| m.len()).unwrap_or(0);
        if file_size == 0 {
            return Err("El server.jar descargado esta vacio".into());
        }

        self.download_progress = 1.0;
        Ok(())
    }

    fn download_fabric(&mut self, path: &str, version: &str) -> Result<(), String> {
        self.download_status = "Consultando API de Fabric...".into();
        self.download_progress = 0.1;

        let loaders_url = "https://meta.fabricmc.net/v2/versions/loader";
        let loaders: serde_json::Value = self
            .http_client
            .get(loaders_url)
            .send()
            .map_err(|e| format!("Error obteniendo loaders: {}", e))?
            .json()
            .map_err(|e| format!("Error parseando loaders: {}", e))?;

        let loader_version = loaders
            .as_array()
            .ok_or("Lista de loaders vacia")?
            .iter()
            .find(|l| l["stable"].as_bool() == Some(true))
            .and_then(|l| l["version"].as_str())
            .ok_or("No se encontro version estable de Fabric")?;

        let jar_url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/server/jar",
            version, loader_version
        );

        self.download_status = format!("Descargando Fabric Loader {}...", loader_version);
        self.download_progress = 0.5;

        let dest = Path::new(path).join("server.jar");
        self.download_file(&jar_url, &dest)?;

        self.download_progress = 1.0;
        Ok(())
    }

    fn download_forge(&mut self, path: &str, version: &str) -> Result<(), String> {
        self.download_status = "Buscando version de Forge...".into();
        self.download_progress = 0.1;

        let promo_url =
            "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
        let promos: serde_json::Value = self
            .http_client
            .get(promo_url)
            .send()
            .map_err(|e| format!("Error obteniendo promos: {}", e))?
            .json()
            .map_err(|e| format!("Error parseando promos: {}", e))?;

        let promos_obj = promos["promos"]
            .as_object()
            .ok_or("Formato de promos invalido")?;

        let forge_version = promos_obj
            .get(&format!("{}-recommended", version))
            .or_else(|| promos_obj.get(&format!("{}-latest", version)))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("No hay version Forge disponible para MC {}", version))?;

        let full_version = format!("{}-{}", version, forge_version);
        let installer_url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
            full_version, full_version
        );

        self.download_status = format!("Descargando instalador Forge {}...", forge_version);
        self.download_progress = 0.3;

        let installer_path = Path::new(path).join("forge-installer.jar");
        self.download_file(&installer_url, &installer_path)?;

        self.download_status = "Ejecutando instalador de Forge...".into();
        self.download_progress = 0.6;

        let output = Command::new("java")
            .arg("-jar")
            .arg("forge-installer.jar")
            .arg("--installServer")
            .arg(".")
            .current_dir(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Error ejecutando instalador: {}", e))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "El instalador de Forge fallo.\nDetalles: {}",
                err_msg
            ));
        }

        self.download_status = "Finalizando...".into();
        self.download_progress = 0.9;

        let has_jar = Path::new(path).join("server.jar").exists();
        let has_run = Path::new(path).join("run.sh").exists();

        let _ = fs::remove_file(&installer_path);

        if !has_jar && !has_run {
            return Err("No se encontro server.jar ni run.sh despues de instalar Forge.".into());
        }

        self.download_progress = 1.0;
        Ok(())
    }

    fn download_neoforge(&mut self, path: &str, version: &str) -> Result<(), String> {
        self.download_status = "Buscando version de NeoForge...".into();
        self.download_progress = 0.1;

        let meta_url =
            "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
        let meta: serde_json::Value = self
            .http_client
            .get(meta_url)
            .send()
            .map_err(|e| format!("Error obteniendo metadata NeoForge: {}", e))?
            .json()
            .map_err(|e| format!("Error parseando metadata: {}", e))?;

        let versions = meta["versions"]
            .as_array()
            .ok_or("Lista de versiones NeoForge vacia")?;

        let parts: Vec<&str> = version.split('.').collect();
        let prefix = if parts.len() >= 3 {
            format!("{}.{}", parts[1], parts[2])
        } else if parts.len() == 2 {
            format!("{}.0", parts[1])
        } else {
            version.to_string()
        };

        let neoforge_version = versions
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|v| v.starts_with(&prefix))
            .last()
            .ok_or_else(|| format!("No hay NeoForge disponible para MC {}", version))?;

        let installer_url = format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
            neoforge_version, neoforge_version
        );

        self.download_status = format!("Descargando instalador NeoForge {}...", neoforge_version);
        self.download_progress = 0.3;

        let installer_path = Path::new(path).join("neoforge-installer.jar");
        self.download_file(&installer_url, &installer_path)?;

        self.download_status = "Ejecutando instalador de NeoForge...".into();
        self.download_progress = 0.6;

        let output = Command::new("java")
            .arg("-jar")
            .arg("neoforge-installer.jar")
            .arg("--installServer")
            .arg(".")
            .current_dir(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Error ejecutando instalador: {}", e))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "El instalador de NeoForge fallo.\nDetalles: {}",
                err_msg
            ));
        }

        self.download_status = "Finalizando...".into();
        self.download_progress = 0.9;

        let has_jar = Path::new(path).join("server.jar").exists();
        let has_run = Path::new(path).join("run.sh").exists();

        let _ = fs::remove_file(&installer_path);

        if !has_jar && !has_run {
            return Err("No se encontro server.jar ni run.sh despues de instalar NeoForge.".into());
        }

        self.download_progress = 1.0;
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        for (name, mut child) in self.running_servers.drain() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "stop");
            }
            let _ = child.wait();
            let _ = name;
        }
    }
}

fn loader_color(loader: &str) -> Color {
    App::get_loaders()
        .iter()
        .find(|l| l.name == loader)
        .map(|l| l.color)
        .unwrap_or(Color::Gray)
}

fn loader_short(loader: &str) -> String {
    App::get_loaders()
        .iter()
        .find(|l| l.name == loader)
        .map(|l| l.short.clone())
        .unwrap_or_else(|| "?".into())
}

fn build_version_line(info: &VersionInfo) -> Line<'static> {
    let mut spans = vec![Span::raw(format!("{:<10}", info.version)), Span::raw(" ")];
    if info.version_type == "snapshot" {
        spans.push(Span::styled(
            "[SNAPSHOT] ",
            Style::default().fg(Color::Yellow),
        ));
    }
    for loader in &info.loaders {
        spans.push(Span::styled(
            format!("[{}] ", loader_short(loader)),
            Style::default()
                .fg(loader_color(loader))
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
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
        .split(vertical[1])[1]
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new();
    let mut running = true;

    while running {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.screen {
                Screen::Main => match key.code {
                    KeyCode::Up => app.index = app.index.saturating_sub(1),
                    KeyCode::Down => app.index = (app.index + 1).min(2),
                    KeyCode::Enter | KeyCode::Char(' ') => match app.index {
                        0 => {
                            app.screen = Screen::Servers;
                            app.index = 0;
                        }
                        1 => {
                            app.screen = Screen::Bridge;
                            app.index = 0;
                        }
                        _ => running = false,
                    },
                    _ => {}
                },

                Screen::Servers => {
                    let has_servers = !app.config.servers.is_empty();
                    let total_nav = if has_servers {
                        app.config.servers.len() + 2
                    } else {
                        2
                    };
                    match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(total_nav - 1),
                        KeyCode::Enter => {
                            if has_servers && app.index < app.config.servers.len() {
                                app.selected_server = Some(app.index);
                                app.screen = Screen::ServerMenu;
                                app.index = 0;
                            } else if (has_servers && app.index == app.config.servers.len())
                                || (!has_servers && app.index == 0)
                            {
                                app.screen = Screen::AddingServer;
                                app.input_buffer.clear();
                                app.error_message = None;
                            } else {
                                app.screen = Screen::Main;
                                app.index = 0;
                            }
                        }
                        KeyCode::Char(' ') => {
                            if has_servers && app.index < app.config.servers.len() {
                                if let Err(e) = app.delete_server(app.index) {
                                    eprintln!("Error: {}", e);
                                }
                                if app.index >= app.config.servers.len() && app.index > 0 {
                                    app.index -= 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                Screen::ServerMenu => {
                    let total = app.get_server_menu_options().len();
                    match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(total - 1),
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if let Some(server_idx) = app.selected_server {
                                let configured = app.is_server_configured(server_idx);
                                let is_running = app.is_server_running(server_idx);

                                if !configured {
                                    match app.index {
                                        0 => {
                                            app.screen = Screen::Setup;
                                            app.setup_phase = SetupPhase::Version;
                                            app.input_buffer.clear();
                                            app.ram_buffer.clear();
                                            app.setup_type_index = 0;
                                            app.error_message = None;
                                            app.filter_versions();
                                        }
                                        1 => {
                                            if let Err(e) = app.delete_server(server_idx) {
                                                eprintln!("Error: {}", e);
                                            }
                                            app.screen = Screen::Servers;
                                            app.selected_server = None;
                                            app.index = 0;
                                        }
                                        _ => {
                                            app.screen = Screen::Servers;
                                            app.index = server_idx;
                                            app.selected_server = None;
                                        }
                                    }
                                } else if is_running {
                                    match app.index {
                                        0 => {
                                            if let Err(e) = app.stop_server(server_idx) {
                                                app.error_message = Some(e);
                                            }
                                        }
                                        1 => {
                                            if let Err(e) = app.delete_server(server_idx) {
                                                eprintln!("Error: {}", e);
                                            }
                                            app.screen = Screen::Servers;
                                            app.selected_server = None;
                                            app.index = 0;
                                        }
                                        _ => {
                                            app.screen = Screen::Servers;
                                            app.index = server_idx;
                                            app.selected_server = None;
                                        }
                                    }
                                } else {
                                    match app.index {
                                        0 => {
                                            if let Err(e) = app.start_server(server_idx) {
                                                app.error_message = Some(e);
                                            }
                                        }
                                        1 => {
                                            if let Err(e) = app.delete_server(server_idx) {
                                                eprintln!("Error: {}", e);
                                            }
                                            app.screen = Screen::Servers;
                                            app.selected_server = None;
                                            app.index = 0;
                                        }
                                        _ => {
                                            app.screen = Screen::Servers;
                                            app.index = server_idx;
                                            app.selected_server = None;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                Screen::Setup => match app.setup_phase {
                    SetupPhase::Version => match key.code {
                        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                            app.input_buffer.push(c);
                            app.error_message = None;
                            app.filter_versions();
                        }
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                            app.error_message = None;
                            app.filter_versions();
                        }
                        KeyCode::Up => {
                            app.version_scroll_index = app.version_scroll_index.saturating_sub(1)
                        }
                        KeyCode::Down => {
                            if !app.filtered_versions.is_empty() {
                                app.version_scroll_index = (app.version_scroll_index + 1)
                                    .min(app.filtered_versions.len() - 1);
                            }
                        }
                        KeyCode::Enter => {
                            if !app.filtered_versions.is_empty() {
                                let idx = app.filtered_versions[app.version_scroll_index];
                                let version = app.version_list[idx].version.clone();
                                let loaders = app.version_list[idx].loaders.clone();
                                if let Some(s) = app.selected_server {
                                    app.config.servers[s].version = Some(version);
                                    app.save();
                                }
                                app.available_loaders = loaders;
                                app.input_buffer.clear();
                                app.ram_buffer.clear();
                                app.error_message = None;
                                app.setup_phase = SetupPhase::Ram;
                            }
                        }
                        KeyCode::Esc => {
                            app.screen = Screen::ServerMenu;
                            app.index = 0;
                            app.input_buffer.clear();
                            app.ram_buffer.clear();
                            app.error_message = None;
                        }
                        _ => {}
                    },

                    SetupPhase::Ram => match key.code {
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            app.ram_buffer.push(c);
                            app.error_message = None;
                        }
                        KeyCode::Backspace => {
                            app.ram_buffer.pop();
                            app.error_message = None;
                        }
                        KeyCode::Enter => match app.parse_ram() {
                            Ok(gb) => {
                                if let Some(idx) = app.selected_server {
                                    app.config.servers[idx].ram_gb = Some(gb);
                                    app.save();
                                }
                                app.ram_buffer.clear();
                                app.error_message = None;
                                app.setup_type_index = 0;
                                app.setup_phase = SetupPhase::Type;
                            }
                            Err(e) => app.error_message = Some(e),
                        },
                        KeyCode::Esc => {
                            app.screen = Screen::ServerMenu;
                            app.index = 0;
                            app.ram_buffer.clear();
                            app.error_message = None;
                        }
                        _ => {}
                    },

                    SetupPhase::Type => {
                        let total = app.available_loaders.len();
                        match key.code {
                            KeyCode::Up => {
                                app.setup_type_index = app.setup_type_index.saturating_sub(1)
                            }
                            KeyCode::Down => {
                                if total > 0 {
                                    app.setup_type_index =
                                        (app.setup_type_index + 1).min(total - 1);
                                }
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if total > 0 {
                                    if let Some(idx) = app.selected_server {
                                        let kind =
                                            app.available_loaders[app.setup_type_index].clone();
                                        app.config.servers[idx].server_type = Some(kind);
                                        app.save();
                                    }
                                    app.screen = Screen::Downloading;
                                    app.download_status = "Iniciando descarga...".into();
                                    app.download_progress = 0.0;
                                    app.download_error = None;
                                    app.download_done = false;
                                    app.download_server();
                                }
                            }
                            KeyCode::Esc => {
                                app.screen = Screen::ServerMenu;
                                app.index = 0;
                            }
                            _ => {}
                        }
                    }
                },

                Screen::Downloading => match key.code {
                    KeyCode::Enter | KeyCode::Esc => {
                        if app.download_done || app.download_error.is_some() {
                            app.screen = Screen::ServerMenu;
                            app.index = 0;
                            app.error_message = app.download_error.take();
                        }
                    }
                    _ => {}
                },

                Screen::Bridge => {
                    let total = 3;
                    match key.code {
                        KeyCode::Up => app.index = app.index.saturating_sub(1),
                        KeyCode::Down => app.index = (app.index + 1).min(total - 1),
                        KeyCode::Enter | KeyCode::Char(' ') => match app.index {
                            0 => {
                                app.config.bridge = Some("playit".into());
                                app.save();
                            }
                            1 => {
                                app.config.bridge = Some("zerotier".into());
                                app.save();
                            }
                            _ => {
                                app.screen = Screen::Main;
                                app.index = 0;
                            }
                        },
                        _ => {}
                    }
                }

                Screen::AddingServer => match key.code {
                    KeyCode::Char(c) => {
                        app.input_buffer.push(c);
                        app.error_message = None;
                    }
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                        app.error_message = None;
                    }
                    KeyCode::Enter => {
                        let name = app.input_buffer.trim().to_string();
                        if name.is_empty() {
                            app.error_message = Some("El nombre no puede estar vacio".to_string());
                        } else if app.server_name_exists(&name) {
                            app.error_message =
                                Some("Ya existe un servidor con ese nombre".to_string());
                        } else {
                            let path = format!("./servidores/{}", name);
                            if let Err(e) = fs::create_dir_all(&path) {
                                app.error_message = Some(format!("Error al crear carpeta: {}", e));
                            } else {
                                app.config.servers.push(Server {
                                    name,
                                    path,
                                    version: None,
                                    server_type: None,
                                    ram_gb: None,
                                });
                                app.save();
                                app.input_buffer.clear();
                                app.error_message = None;
                                app.screen = Screen::Servers;
                                app.index = app.config.servers.len() - 1;
                            }
                        }
                    }
                    KeyCode::Esc => {
                        app.input_buffer.clear();
                        app.error_message = None;
                        app.screen = Screen::Servers;
                    }
                    _ => {}
                },
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    match app.screen {
        Screen::Main => {
            let items = ["Gestionar Servidores", "Puente", "Salir"];
            let list = List::new(items.iter().map(|i| Line::from(*i)).collect::<Vec<_>>())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Menu Principal"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);
        }

        Screen::Servers => {
            let display = app.get_display_items();
            let items: Vec<Line> = display.iter().map(|i| Line::from(i.as_str())).collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Servidores (Enter: opciones, Espacio: borrar)"),
                )
                .highlight_style(Style::default().fg(Color::Cyan))
                .highlight_symbol(">> ");
            let real_index = if app.config.servers.is_empty() {
                app.index + 1
            } else {
                app.index
            };
            let safe = real_index.min(display.len().saturating_sub(1));
            let mut state = ListState::default().with_selected(Some(safe));
            f.render_stateful_widget(list, area, &mut state);
        }

        Screen::ServerMenu => {
            let name = app
                .selected_server
                .map(|idx| app.config.servers[idx].name.clone())
                .unwrap_or_else(|| "Desconocido".into());
            let options = app.get_server_menu_options();
            let list = List::new(
                options
                    .iter()
                    .map(|i| Line::from(i.as_str()))
                    .collect::<Vec<_>>(),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Opciones de: {}", name)),
            )
            .highlight_style(Style::default().fg(Color::Magenta))
            .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);

            if let Some(ref err) = app.error_message {
                let err_area = centered_rect(70, 40, area);
                f.render_widget(Clear, err_area);
                let err_text: Vec<Line> = err
                    .lines()
                    .map(|l| {
                        Line::from(Span::styled(
                            l.to_string(),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ))
                    })
                    .collect();
                let err_para = Paragraph::new(err_text)
                    .block(Block::default().borders(Borders::ALL).title("Error"));
                f.render_widget(err_para, err_area);
            }
        }

        Screen::Setup => match app.setup_phase {
            SetupPhase::Version => {
                let popup_area = centered_rect(75, 70, area);
                f.render_widget(Clear, popup_area);
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(5), Constraint::Min(0)])
                    .split(popup_area);

                let input_text = vec![
                    Line::from("Paso 1/3: Selecciona la version (filtra con numeros/puntos)"),
                    Line::from(Span::styled(
                        format!("Buscar: {}_", app.input_buffer),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                ];
                let input_widget = Paragraph::new(input_text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Filtrar Versiones"),
                );
                f.render_widget(input_widget, layout[0]);

                let items: Vec<Line> = app
                    .filtered_versions
                    .iter()
                    .map(|&idx| build_version_line(&app.version_list[idx]))
                    .collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        "Versiones de Mojang ({})",
                        app.filtered_versions.len()
                    )))
                    .highlight_style(
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                let mut state = ListState::default().with_selected(Some(app.version_scroll_index));
                f.render_stateful_widget(list, layout[1], &mut state);
            }
            SetupPhase::Ram => {
                let ram_area = centered_rect(60, 50, area);
                f.render_widget(Clear, ram_area);
                let mut text = vec![
                    Line::from(format!(
                        "Paso 2/3: Memoria RAM (version {})",
                        app.selected_version()
                    )),
                    Line::from(""),
                    Line::from("Indica cuantos GB de RAM tendra este servidor."),
                    Line::from("Usa numeros enteros, sin la letra GB (ej: 2, 4, 8)"),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("RAM (GB): {}_", app.ram_buffer),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("Enter: continuar | Esc: cancelar"),
                ];
                if let Some(ref err) = app.error_message {
                    text.push(Line::from(""));
                    text.push(Line::from(Span::styled(
                        format!("Error: {}", err),
                        Style::default().fg(Color::Red),
                    )));
                }
                let paragraph = Paragraph::new(text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Configurar RAM"),
                );
                f.render_widget(paragraph, ram_area);
            }
            SetupPhase::Type => {
                let popup_area = centered_rect(70, 60, area);
                f.render_widget(Clear, popup_area);
                let items: Vec<Line> = app
                    .available_loaders
                    .iter()
                    .map(|l| {
                        Line::from(Span::styled(
                            l.clone(),
                            Style::default()
                                .fg(loader_color(l))
                                .add_modifier(Modifier::BOLD),
                        ))
                    })
                    .collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        "Paso 3/3: Tipo para v{} | RAM: {} (Enter: confirmar, Esc: cancelar)",
                        app.selected_version(),
                        app.selected_ram()
                    )))
                    .highlight_style(
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                let mut state = ListState::default().with_selected(Some(app.setup_type_index));
                f.render_stateful_widget(list, popup_area, &mut state);
            }
        },

        Screen::Downloading => {
            let popup_area = centered_rect(70, 40, area);
            f.render_widget(Clear, popup_area);

            let mut text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Descargando servidor...",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(app.download_status.clone()),
                Line::from(""),
            ];

            let bar_width = 50;
            let filled = (app.download_progress * bar_width as f32) as usize;
            let empty = bar_width as usize - filled;
            let bar = format!(
                "[{}{}] {:.0}%",
                "=".repeat(filled),
                " ".repeat(empty),
                app.download_progress * 100.0
            );
            text.push(Line::from(Span::styled(
                bar,
                Style::default().fg(Color::Cyan),
            )));

            if let Some(ref err) = app.download_error {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    format!("Error: {}", err),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                text.push(Line::from(""));
                text.push(Line::from("Presiona Enter para volver"));
            } else if app.download_done {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Completado!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
                text.push(Line::from(""));
                text.push(Line::from("Presiona Enter para continuar"));
            }

            let paragraph = Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Descarga en progreso"),
            );
            f.render_widget(paragraph, popup_area);
        }

        Screen::Bridge => {
            let current = app.config.bridge.as_deref().unwrap_or("Ninguno");
            let items = [
                format!(
                    "Playit {}",
                    if current == "playit" { "[ACTIVO]" } else { "" }
                ),
                format!(
                    "ZeroTier {}",
                    if current == "zerotier" {
                        "[ACTIVO]"
                    } else {
                        ""
                    }
                ),
                "[<] Volver al Menu".to_string(),
            ];
            let list = List::new(
                items
                    .iter()
                    .map(|i| Line::from(i.as_str()))
                    .collect::<Vec<_>>(),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Puente (Actual: {})", current)),
            )
            .highlight_style(Style::default().fg(Color::Green))
            .highlight_symbol(">> ");
            let mut state = ListState::default().with_selected(Some(app.index));
            f.render_stateful_widget(list, area, &mut state);
        }

        Screen::AddingServer => {
            let popup_area = centered_rect(60, 30, area);
            f.render_widget(Clear, popup_area);
            let mut text = vec![
                Line::from("Nombre del servidor:"),
                Line::from(""),
                Line::from(Span::styled(
                    format!("> {}_", app.input_buffer),
                    Style::default().fg(Color::White),
                )),
            ];
            if let Some(ref err) = app.error_message {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    format!("Error: {}", err),
                    Style::default().fg(Color::Red),
                )));
            }
            let paragraph = Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Agregar Nuevo Servidor"),
            );
            f.render_widget(paragraph, popup_area);
        }
    }
}
