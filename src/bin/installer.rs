use clap::Parser;
use inquire::Select;
use inquire::Confirm;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs::Permissions;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const INIT_SYSTEMD: &str = include_str!("../../init/mc-manager.service");
const INIT_OPENRC: &str = include_str!("../../init/mc-manager.openrc");
const INIT_SYSV: &str = include_str!("../../init/mc-manager.sysv");
const INIT_DINIT: &str = include_str!("../../init/mc-manager.dinit");
const INIT_RUNIT: &str = include_str!("../../init/runit/run");
const INIT_DEFAULT: &str = include_str!("../../init/mc-manager.default");

const GITHUB_REPO: &str = "FerNandoGD24/Minecraft-Server-Manager-Rust";

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Parser)]
#[command(name = "mc-manager-installer", about = "Instala Minecraft Server Manager")]
struct Cli {
    #[arg(long)]
    method: Option<String>,
    #[arg(long, help = "Desinstala mc-manager completamente")]
    uninstall: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.uninstall {
        return uninstall();
    }
    if let Some(m) = cli.method {
        match m.as_str() {
            "precompiled" => return install_precompiled(),
            "source" => return install_from_source(),
            _ => {}
        }
    }
    run_interactive()
}

fn run_interactive() -> anyhow::Result<()> {
    let options = vec!["Precompilado (rapido)", "Desde fuente (compilar)"];
    let choice = Select::new("Como instalar Minecraft Server Manager?", options).prompt()?;
    if choice == "Precompilado (rapido)" {
        install_precompiled()
    } else {
        install_from_source()
    }
}

fn install_precompiled() -> anyhow::Result<()> {
    let asset = match std::env::consts::ARCH {
        "x86_64" => "mc-manager-linux-x86_64",
        "aarch64" => "mc-manager-linux-aarch64",
        _ => anyhow::bail!("Arquitectura no soportada"),
    };

    // Fetch latest release version from GitHub API
    let (version, download_url) = fetch_latest_release(asset)?;
    println!("Descargando v{}...", version);

    let dest = PathBuf::from("/tmp/mc-manager.new");
    let client = Client::new();
    let mut resp = client.get(&download_url).send()?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&dest)?);
    std::io::copy(&mut resp, &mut out)?;
    out.flush()?;
    install_and_configure(&dest)
}

fn fetch_latest_release(asset_name: &str) -> anyhow::Result<(String, String)> {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/FerNandoGD24/Minecraft-Server-Manager-Rust/releases/latest");
    let release: GitHubRelease = client
        .get(&url)
        .header("User-Agent", "mc-manager-installer")
        .send()?
        .json()?;

    let version = release.tag_name.trim_start_matches('v').to_string();
    let asset = release.assets.iter().find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow::anyhow!("Asset {} no encontrado en release", asset_name))?;

    Ok((version, asset.browser_download_url.clone()))
}

fn install_from_source() -> anyhow::Result<()> {
    ensure_rust_installed()?;
    ensure_git_installed()?;
    println!("Clonando repositorio...");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", "--branch", "main", "https://github.com/FerNandoGD24/Minecraft-Server-Manager-Rust.git", "/tmp/mc-manager-src"])
        .status()?;
    if !status.success() {
        anyhow::bail!("Fallo git clone");
    }
    println!("Compilando release...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir("/tmp/mc-manager-src")
        .status()?;
    if !status.success() {
        anyhow::bail!("Fallo compilacion");
    }
    let bin = PathBuf::from("/tmp/mc-manager-src/target/release/mc-manager");
    install_and_configure(&bin)
}

fn ensure_rust_installed() -> anyhow::Result<()> {
    if Command::new("cargo").arg("--version").output()?.status.success() {
        return Ok(());
    }
    println!("Instalando Rust via rustup...");
    let status = Command::new("sh")
        .arg("-c")
        .arg("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable")
        .status()?;
    if !status.success() {
        anyhow::bail!("Fallo instalando Rust");
    }
    Ok(())
}

fn ensure_git_installed() -> anyhow::Result<()> {
    if !Command::new("git").arg("--version").output()?.status.success() {
        anyhow::bail!("Git requerido");
    }
    Ok(())
}

fn install_and_configure(binary: &PathBuf) -> anyhow::Result<()> {
    let dest = PathBuf::from("/usr/local/bin/mc-manager");
    std::fs::copy(binary, &dest)?;
    std::fs::set_permissions(&dest, Permissions::from_mode(0o755))?;
    configure_init_system()?;
    create_user_and_dirs()?;
    verify_installation()?;
    Ok(())
}

fn configure_init_system() -> anyhow::Result<()> {
    println!("\n--- Configuracion init system ---");

    let init_system = detect_init_system();
    println!("Init system detectado: {}", init_system);

    let confirm = Confirm::new("Configurar auto-start (servicio systemd/OpenRC/runit/dinit/SysV)?")
        .with_default(true)
        .prompt()?;

    if !confirm {
        println!("Auto-start omitido. Puedes configurarlo luego con los archivos en /usr/share/mc-manager/init/");
        install_init_templates()?;
        return Ok(());
    }

    match init_system.as_str() {
        "systemd" => configure_systemd()?,
        "openrc" => configure_openrc()?,
        "runit" => configure_runit()?,
        "dinit" => configure_dinit()?,
        "sysv" => configure_sysv()?,
        _ => {
            println!("Init system no soportado automaticamente: {}", init_system);
            install_init_templates()?;
        }
    }

    println!("Auto-start configurado correctamente");
    Ok(())
}

fn detect_init_system() -> String {
    if PathBuf::from("/run/systemd/system").exists() {
        return "systemd".to_string();
    }
    if Command::new("openrc").arg("--version").output().is_ok() {
        return "openrc".to_string();
    }
    if PathBuf::from("/etc/runit").exists() || PathBuf::from("/var/service").exists() {
        return "runit".to_string();
    }
    if PathBuf::from("/etc/dinit.d").exists() {
        return "dinit".to_string();
    }
    if PathBuf::from("/etc/init.d").exists() {
        return "sysv".to_string();
    }
    "unknown".to_string()
}

fn install_init_templates() -> anyhow::Result<()> {
    let dest = PathBuf::from("/usr/share/mc-manager/init");
    std::fs::create_dir_all(&dest)?;

    std::fs::write(dest.join("mc-manager.service"), INIT_SYSTEMD)?;
    std::fs::write(dest.join("mc-manager.openrc"), INIT_OPENRC)?;
    std::fs::write(dest.join("mc-manager.sysv"), INIT_SYSV)?;
    std::fs::write(dest.join("mc-manager.dinit"), INIT_DINIT)?;
    std::fs::write(dest.join("runit.run"), INIT_RUNIT)?;
    std::fs::write(dest.join("mc-manager.default"), INIT_DEFAULT)?;

    println!("Templates init instalados en: {}", dest.display());
    Ok(())
}

fn configure_systemd() -> anyhow::Result<()> {
    let dest = PathBuf::from("/etc/systemd/system/mc-manager.service");
    let content = INIT_SYSTEMD.replace("%i", "minecraft");
    std::fs::write(&dest, content)?;
    Command::new("systemctl").args(["daemon-reload"]).status()?;
    Command::new("systemctl").args(["enable", "mc-manager"]).status()?;
    Ok(())
}

fn configure_openrc() -> anyhow::Result<()> {
    let dest = PathBuf::from("/etc/init.d/mc-manager");
    std::fs::write(&dest, INIT_OPENRC)?;
    std::fs::set_permissions(&dest, Permissions::from_mode(0o755))?;
    Command::new("rc-update").args(["add", "mc-manager", "default"]).status()?;
    Ok(())
}

fn configure_runit() -> anyhow::Result<()> {
    let sv_dir = PathBuf::from("/etc/sv/mc-manager");
    std::fs::create_dir_all(&sv_dir)?;
    std::fs::write(sv_dir.join("run"), INIT_RUNIT)?;
    std::fs::set_permissions(sv_dir.join("run"), Permissions::from_mode(0o755))?;
    let service_dir = PathBuf::from("/var/service");
    if service_dir.exists() {
        let _ = std::fs::remove_file(service_dir.join("mc-manager"));
        std::os::unix::fs::symlink(&sv_dir, service_dir.join("mc-manager"))?;
    }
    Ok(())
}

fn configure_dinit() -> anyhow::Result<()> {
    let dest = PathBuf::from("/etc/dinit.d/mc-manager");
    std::fs::write(&dest, INIT_DINIT)?;
    Command::new("dinitctl").args(["enable", "mc-manager"]).status()?;
    Ok(())
}

fn configure_sysv() -> anyhow::Result<()> {
    let dest = PathBuf::from("/etc/init.d/mc-manager");
    std::fs::write(&dest, INIT_SYSV)?;
    std::fs::set_permissions(&dest, Permissions::from_mode(0o755))?;
    let default_dest = PathBuf::from("/etc/default/mc-manager");
    std::fs::write(&default_dest, INIT_DEFAULT)?;
    if Command::new("update-rc.d").arg("--version").output().is_ok() {
        Command::new("update-rc.d").args(["mc-manager", "defaults"]).status()?;
    } else if Command::new("chkconfig").arg("--version").output().is_ok() {
        Command::new("chkconfig").args(["--add", "mc-manager"]).status()?;
    }
    Ok(())
}

fn create_user_and_dirs() -> anyhow::Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/minecraft"));
    std::fs::create_dir_all(format!("{}/minecraft-servers", home.display()))?;
    std::fs::create_dir_all(format!("{}/.mc-manager", home.display()))?;
    Ok(())
}

fn verify_installation() -> anyhow::Result<()> {
    println!("Verificando instalacion...");
    Command::new("mc-manager").arg("--help").output()?;
    println!("Instalacion completada. Usa: mc-manager");
    Ok(())
}
fn uninstall() -> anyhow::Result<()> {
    println!("=== Desinstalando Minecraft Server Manager ===\n");

    let confirm = Confirm::new("Esto eliminara TODO: binarios, servicios, configs y datos de usuario.\nContinuar?")
        .with_default(false)
        .prompt()?;

    if !confirm {
        println!("Desinstalacion cancelada.");
        return Ok(());
    }

    println!("\n1. Deteniendo y deshabilitando servicios...");

    // systemd
    let _ = Command::new("systemctl").args(["stop", "mc-manager"]).status();
    let _ = Command::new("systemctl").args(["disable", "mc-manager"]).status();
    let _ = std::fs::remove_file("/etc/systemd/system/mc-manager.service");
    let _ = Command::new("systemctl").args(["daemon-reload"]).status();

    // OpenRC
    let _ = Command::new("rc-service").args(["mc-manager", "stop"]).status();
    let _ = Command::new("rc-update").args(["del", "mc-manager", "default"]).status();
    let _ = std::fs::remove_file("/etc/init.d/mc-manager");

    // runit
    let _ = std::fs::remove_file("/var/service/mc-manager");
    let _ = std::fs::remove_dir_all("/etc/sv/mc-manager");

    // dinit
    let _ = Command::new("dinitctl").args(["stop", "mc-manager"]).status();
    let _ = Command::new("dinitctl").args(["disable", "mc-manager"]).status();
    let _ = std::fs::remove_file("/etc/dinit.d/mc-manager");

    // SysV
    let _ = Command::new("/etc/init.d/mc-manager").arg("stop").status();
    let _ = std::fs::remove_file("/etc/init.d/mc-manager");
    let _ = std::fs::remove_file("/etc/default/mc-manager");
    let _ = Command::new("update-rc.d").args(["mc-manager", "remove"]).status();
    let _ = Command::new("chkconfig").args(["--del", "mc-manager"]).status();

    println!("2. Eliminando binarios...");
    let _ = std::fs::remove_file("/usr/local/bin/mc-manager");
    let _ = std::fs::remove_file("/usr/local/bin/mc-manager-installer");

    println!("3. Eliminando templates init...");
    let _ = std::fs::remove_dir_all("/usr/share/mc-manager");

    println!("4. Eliminando configuracion de usuario...");
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/minecraft"));
    let mc_manager_dir = home.join(".mc-manager");
    if mc_manager_dir.exists() {
        let keep_data = Confirm::new("Eliminar tambien datos de usuario (~/.mc-manager) incluyendo configs de servidores?")
            .with_default(false)
            .prompt()?;
        if keep_data {
            std::fs::remove_dir_all(&mc_manager_dir)?;
            println!("   Datos de usuario eliminados.");
        } else {
            println!("   Datos de usuario conservados en: {}", mc_manager_dir.display());
        }
    }

    println!("\n=== Desinstalacion completada ===");
    Ok(())
}