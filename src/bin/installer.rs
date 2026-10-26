use clap::Parser;
use inquire::Select;
use inquire::Confirm;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs::Permissions;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::io::IsTerminal;

const INIT_SYSTEMD: &str = include_str!("../../init/mc-manager.service");
const INIT_OPENRC: &str = include_str!("../../init/mc-manager.openrc");
const INIT_SYSV: &str = include_str!("../../init/mc-manager.sysv");
const INIT_DINIT: &str = include_str!("../../init/mc-manager.dinit");
const INIT_RUNIT: &str = include_str!("../../init/runit/run");
const INIT_DEFAULT: &str = include_str!("../../init/mc-manager.default");

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
    #[arg(long, help = "Instalación system-wide (requiere sudo)")]
    system: bool,
    #[arg(long, help = "Instalación en espacio de usuario (default)")]
    user: bool,
    #[arg(long)]
    method: Option<String>,
    #[arg(long, help = "Desinstala mc-manager completamente")]
    uninstall: bool,
}

#[derive(Debug, Clone)]
struct InstallContext {
    system: bool,
    bin_dir: PathBuf,
    service_dir: PathBuf,
    init_dir: PathBuf,
    data_dir: PathBuf,
    needs_root: bool,
}

impl InstallContext {
    fn new(system: bool) -> Self {
        let system = if std::env::var("USER").unwrap_or_default() == "root" {
            true
        } else {
            system
        };

        let (bin_dir, service_dir, init_dir, data_dir) = if system {
            (
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/etc/systemd/system"),
                PathBuf::from("/usr/share/mc-manager/init"),
                PathBuf::from("/var/lib/mc-manager"),
            )
        } else {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/minecraft"));
            (
                home.join(".local/bin"),
                home.join(".config/systemd/user"),
                home.join(".local/share/mc-manager/init"),
                home.join(".local/share/mc-manager"),
            )
        };

        let needs_root = system && !is_root();

        Self {
            system,
            bin_dir,
            service_dir,
            init_dir,
            data_dir,
            needs_root,
        }
    }

    fn is_user_mode(&self) -> bool {
        !self.system
    }
}

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn run_privileged_cmd(cmd: &[&str]) -> anyhow::Result<()> {
    if is_root() {
        Command::new(cmd[0]).args(&cmd[1..]).status()?;
        return Ok(());
    }

    if std::io::stdin().is_terminal() {
        let status = Command::new("sudo").args(cmd).status()?;
        if !status.success() {
            anyhow::bail!("sudo failed");
        }
    } else {
        let status = Command::new("pkexec").args(cmd).status()?;
        if !status.success() {
            anyhow::bail!("pkexec failed");
        }
    }
    Ok(())
}

fn run_privileged_cmd_str(cmd: &str) -> anyhow::Result<()> {
    if is_root() {
        Command::new("bash").args(["-c", cmd]).status()?;
        return Ok(());
    }

    if std::io::stdin().is_terminal() {
        let status = Command::new("sudo").args(["bash", "-c", cmd]).status()?;
        if !status.success() {
            anyhow::bail!("sudo failed");
        }
    } else {
        let status = Command::new("pkexec").args(["bash", "-c", cmd]).status()?;
        if !status.success() {
            anyhow::bail!("pkexec failed");
        }
    }
    Ok(())
}

fn ensure_dirs(ctx: &InstallContext) -> anyhow::Result<()> {
    if ctx.needs_root {
        run_privileged_cmd_str(&format!("mkdir -p {} {} {}", 
            ctx.bin_dir.display(), 
            ctx.service_dir.display(), 
            ctx.init_dir.display()
        ))?;
    } else {
        std::fs::create_dir_all(&ctx.bin_dir)?;
        std::fs::create_dir_all(&ctx.service_dir)?;
        std::fs::create_dir_all(&ctx.init_dir)?;
    }
    std::fs::create_dir_all(&ctx.data_dir)?;
    Ok(())
}

fn copy_with_privilege(src: &Path, dst: &Path, ctx: &InstallContext) -> anyhow::Result<()> {
    if ctx.needs_root {
        run_privileged_cmd(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()])?;
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn write_file_privileged(path: &Path, content: &str, ctx: &InstallContext) -> anyhow::Result<()> {
    if ctx.needs_root {
        let tmp = format!("/tmp/{}", path.file_name().unwrap().to_string_lossy());
        std::fs::write(&tmp, content)?;
        run_privileged_cmd(&["mv", &tmp, path.to_str().unwrap()])?;
    } else {
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn install_service_files(ctx: &InstallContext) -> anyhow::Result<()> {
    ensure_dirs(&InstallContext { init_dir: ctx.init_dir.clone(), needs_root: true, ..InstallContext::new(true) })?;
    
    write_file_privileged(&ctx.init_dir.join("mc-manager.service"), INIT_SYSTEMD, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.openrc"), INIT_OPENRC, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.sysv"), INIT_SYSV, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.dinit"), INIT_DINIT, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("runit.run"), INIT_RUNIT, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.default"), INIT_DEFAULT, &InstallContext::new(true))?;

    if InstallContext::new(true).system {
        run_privileged_cmd(&["systemctl", "daemon-reload"])?;
        run_privileged_cmd(&["systemctl", "enable", "mc-manager"])?;
    } else {
        run_privileged_cmd(&["systemctl", "--user", "daemon-reload"])?;
        run_privileged_cmd(&["systemctl", "--user", "enable", "mc-manager"])?;
        let _ = Command::new("loginctl").args(["enable-linger"]).status();
    }

    println!("Auto-start configurado correctamente ({})", if InstallContext::new(true).system { "system" } else { "user" });
    Ok(())
}

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
    #[arg(long, help = "Instalación system-wide (requiere sudo)")]
    system: bool,
    #[arg(long, help = "Instalación en espacio de usuario (default)")]
    user: bool,
    #[arg(long)]
    method: Option<String>,
    #[arg(long, help = "Desinstala mc-manager completamente")]
    uninstall: bool,
}

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
    #[arg(long, help = "Instalación system-wide (requiere sudo)")]
    system: bool,
    #[arg(long, help = "Instalación en espacio de usuario (default)")]
    user: bool,
    #[arg(long)]
    method: Option<String>,
    #[arg(long, help = "Desinstala mc-manager completamente")]
    uninstall: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.system && cli.user {
        anyhow::bail!("No se puede usar --system y --user a la vez");
    }

    let system = cli.system || (!cli.user && std::env::var("USER").unwrap_or_default() == "root");
    let ctx = InstallContext::new(system);

    if cli.uninstall {
        return uninstall(&InstallContext::new(true));
    }

    if let Some(m) = cli.method {
        match m.as_str() {
            "precompiled" => return install_precompiled(&InstallContext::new(true)),
            "source" => return install_from_source(&InstallContext::new(true)),
            _ => {}
        }
    }
    run_interactive(&InstallContext::new(true))
}

fn run_interactive(ctx: &InstallContext) -> anyhow::Result<()> {
    let mode_str = if ctx.system { "system-wide" } else { "usuario" };
    println!("Modo de instalación: {}", mode_str);

    let options = vec!["Precompilado (rapido)", "Desde fuente (compilar)"];
    let choice = Select::new("Como instalar Minecraft Server Manager?", options).prompt()?;
    if choice == "Precompilado (rapido)" {
        install_precompiled(&InstallContext::new(true))
    } else {
        install_from_source(&InstallContext::new(true))
    }
}

fn install_precompiled(ctx: &InstallContext) -> anyhow::Result<()> {
    let asset = match std::env::consts::ARCH {
        "x86_64" => "mc-manager-linux-x86_64",
        "aarch64" => "mc-manager-linux-aarch64",
        _ => anyhow::bail!("Arquitectura no soportada"),
    };

    let (version, download_url) = fetch_latest_release(asset)?;
    println!("Descargando v{}...", version);
    println!("URL: {}", download_url);

    let dest = PathBuf::from("/tmp/mc-manager.new");
    let client = Client::builder()
        .user_agent("mc-manager-installer")
        .build()?;
    println!("Client created, sending request...");
    let mut resp = Client::builder()
        .user_agent("mc-manager-installer")
        .build()?
        .get(&download_url)
        .send()?;
    println!("Response status: {}", resp.status());
    if !resp.status().is_success() {
        anyhow::bail!("Download failed with status: {}", resp.status());
    }
    println!("Creating output file...");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&dest)?);
    println!("Copying data...");
    std::io::copy(&mut resp, &mut out)?;
    println!("Flush...");
    out.flush()?;
    println!("Download complete");

    install_binary(&dest, &InstallContext::new(true))
}

fn install_binary(binary_path: &Path, ctx: &InstallContext) -> anyhow::Result<()> {
    ensure_dirs(ctx)?;
    
    let dest_binary = ctx.bin_dir.join("mc-manager");
    println!("Instalando binario en: {}", dest_binary.display());
    copy_with_privilege(binary_path, &dest_binary, ctx)?;
    
    std::fs::set_permissions(&dest_binary, Permissions::from_mode(0o755))?;
    
    install_service_files(&InstallContext::new(true))?;
    
    verify_installation(&dest_binary)
}

fn ensure_dirs(ctx: &InstallContext) -> anyhow::Result<()> {
    if ctx.needs_root {
        run_privileged_cmd_str(&format!("mkdir -p {} {} {}", 
            ctx.bin_dir.display(), 
            ctx.service_dir.display(), 
            ctx.init_dir.display()
        ))?;
    } else {
        std::fs::create_dir_all(&ctx.bin_dir)?;
        std::fs::create_dir_all(&ctx.service_dir)?;
        std::fs::create_dir_all(&ctx.init_dir)?;
    }
    std::fs::create_dir_all(&ctx.data_dir)?;
    Ok(())
}

fn copy_with_privilege(src: &Path, dst: &Path, ctx: &InstallContext) -> anyhow::Result<()> {
    if ctx.needs_root {
        run_privileged_cmd(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()])?;
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn write_file_privileged(path: &Path, content: &str, ctx: &InstallContext) -> anyhow::Result<()> {
    if ctx.needs_root {
        let tmp = format!("/tmp/{}", path.file_name().unwrap().to_string_lossy());
        std::fs::write(&tmp, content)?;
        run_privileged_cmd(&["mv", &tmp, path.to_str().unwrap()])?;
    } else {
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn install_service_files(ctx: &InstallContext) -> anyhow::Result<()> {
    ensure_dirs(&InstallContext { init_dir: ctx.init_dir.clone(), needs_root: true, ..InstallContext::new(true) })?;
    
    write_file_privileged(&ctx.init_dir.join("mc-manager.service"), INIT_SYSTEMD, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.openrc"), INIT_OPENRC, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.sysv"), INIT_SYSV, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.dinit"), INIT_DINIT, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("runit.run"), INIT_RUNIT, &InstallContext::new(true))?;
    write_file_privileged(&ctx.init_dir.join("mc-manager.default"), INIT_DEFAULT, &InstallContext::new(true))?;

    if InstallContext::new(true).system {
        run_privileged_cmd(&["systemctl", "daemon-reload"])?;
        run_privileged_cmd(&["systemctl", "enable", "mc-manager"])?;
    } else {
        run_privileged_cmd(&["systemctl", "--user", "daemon-reload"])?;
        run_privileged_cmd(&["systemctl", "--user", "enable", "mc-manager"])?;
        let _ = Command::new("loginctl").args(["enable-linger"]).status();
    }

    println!("Auto-start configurado correctamente ({})", if InstallContext::new(true).system { "system" } else { "user" });
    Ok(())
}

fn fetch_latest_release(asset_name: &str) -> anyhow::Result<(String, String)> {
    let client = Client::builder()
        .user_agent("mc-manager-installer")
        .build()?;
    let url = format!("https://api.github.com/repos/FerNandoGD24/Minecraft-Server-Manager-Rust/releases/latest");
    let release: GitHubRelease = Client::builder()
        .user_agent("mc-manager-installer")
        .build()?
        .get(&format!("https://api.github.com/repos/FerNandoGD24/Minecraft-Server-Manager-Rust/releases/latest"))
        .header("User-Agent", "mc-manager-installer")
        .send()?
        .json()?;

    let version = release.tag_name.trim_start_matches('v').to_string();
    let asset = release.assets.iter().find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow::anyhow!("Asset {} no encontrado en release", asset_name))?;

    Ok((version, asset.browser_download_url.clone()))
}

fn install_from_source(ctx: &InstallContext) -> anyhow::Result<()> {
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
    install_binary(&bin, &InstallContext::new(true))
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

fn verify_installation(binary: &Path) -> anyhow::Result<()> {
    println!("Verificando instalacion...");
    Command::new(binary).arg("--help").output()?;
    println!("Instalacion completada. Usa: mc-manager");
    Ok(())
}

fn uninstall(ctx: &InstallContext) -> anyhow::Result<()> {
    println!("=== Desinstalando Minecraft Server Manager ===\n");

    let confirm = Confirm::new("Esto eliminara binarios, servicios y configs.\nContinuar?")
        .with_default(false)
        .prompt()?;

    if !confirm {
        println!("Desinstalacion cancelada.");
        return Ok(());
    }

    println!("\n1. Deteniendo y deshabilitando servicios...");

    if ctx.system {
        run_privileged_cmd(&["systemctl", "stop", "mc-manager"])?;
        run_privileged_cmd(&["systemctl", "disable", "mc-manager"])?;
        run_privileged_cmd(&["rm", "-f", "/etc/systemd/system/mc-manager.service"])?;
        run_privileged_cmd(&["systemctl", "daemon-reload"])?;
    } else {
        run_privileged_cmd(&["systemctl", "--user", "stop", "mc-manager"])?;
        run_privileged_cmd(&["systemctl", "--user", "disable", "mc-manager"])?;
        run_privileged_cmd(&["rm", "-f", &dirs::home_dir().unwrap().join(".config/systemd/user/mc-manager.service").to_string_lossy()])?;
        run_privileged_cmd(&["systemctl", "--user", "daemon-reload"])?;
    }

    // OpenRC
    run_privileged_cmd(&["rc-service", "mc-manager", "stop"])?;
    run_privileged_cmd(&["rc-update", "del", "mc-manager", "default"])?;
    run_privileged_cmd(&["rm", "-f", "/etc/init.d/mc-manager"])?;

    // runit
    run_privileged_cmd(&["rm", "-f", "/var/service/mc-manager"])?;
    run_privileged_cmd(&["rm", "-rf", "/etc/sv/mc-manager"])?;

    // dinit
    run_privileged_cmd(&["dinitctl", "stop", "mc-manager"])?;
    run_privileged_cmd(&["dinitctl", "disable", "mc-manager"])?;
    run_privileged_cmd(&["rm", "-f", "/etc/dinit.d/mc-manager"])?;

    // SysV
    run_privileged_cmd(&["/etc/init.d/mc-manager", "stop"])?;
    run_privileged_cmd(&["rm", "-f", "/etc/init.d/mc-manager"])?;
    run_privileged_cmd(&["rm", "-f", "/etc/default/mc-manager"])?;
    run_privileged_cmd(&["update-rc.d", "mc-manager", "remove"])?;
    run_privileged_cmd(&["chkconfig", "--del", "mc-manager"])?;

    println!("2. Eliminando binarios...");
    run_privileged_cmd(&["rm", "-f", "/usr/local/bin/mc-manager"])?;
    run_privileged_cmd(&["rm", "-f", "/usr/local/bin/mc-manager-installer"])?;
    run_privileged_cmd(&["rm", "-f", &dirs::home_dir().unwrap().join(".local/bin/mc-manager").to_string_lossy()])?;
    run_privileged_cmd(&["rm", "-f", &dirs::home_dir().unwrap().join(".local/bin/mc-manager-installer").to_string_lossy()])?;

    println!("3. Eliminando templates init...");
    run_privileged_cmd(&["rm", "-rf", "/usr/share/mc-manager"])?;
    run_privileged_cmd(&["rm", "-rf", &dirs::home_dir().unwrap().join(".local/share/mc-manager").to_string_lossy()])?;

    println!("4. Eliminando configuracion de usuario...");
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/minecraft"));
    let mc_manager_dir = home.join(".mc-manager");
    if mc_manager_dir.exists() {
        let keep_data = Confirm::new("Eliminar tambien datos de usuario (~/.mc-manager) incluyendo configs de servidores?")
            .with_default(false)
            .prompt()?;
        if keep_data {
            run_privileged_cmd(&["rm", "-rf", &mc_manager_dir.to_string_lossy()])?;
            println!("   Datos de usuario eliminados.");
        } else {
            println!("   Datos de usuario conservados en: {}", mc_manager_dir.display());
        }
    }

    println!("\n=== Desinstalacion completada ===");
    Ok(())
}