# Init Scripts para mc-manager

Cuatro sistemas de init soportados:

## 1. systemd (recomendado - Ubuntu 16.04+, Debian 8+, RHEL 7+, Arch, Fedora, etc.)

### Instalación
```bash
# Copiar unit file
sudo cp mc-manager.service /etc/systemd/system/

# Ajustar usuario en el archivo (reemplazar %i por tu usuario)
sudo sed -i 's/%i/tu_usuario/g' /etc/systemd/system/mc-manager.service

# Recargar systemd
sudo systemctl daemon-reload

# Habilitar y arrancar
sudo systemctl enable --now mc-manager

# Ver logs
sudo journalctl -u mc-manager -f
```

### Características
- Type=forking (daemon hace double-fork)
- PIDFile automático
- Restart=on-failure
- Hardening: PrivateTmp, ProtectSystem, NoNewPrivileges
- Límites NOFILE/NPROC

---

## 2. OpenRC (Gentoo, Alpine, Artix, Devuan, etc.)

### Instalación
```bash
# Copiar script
sudo cp mc-manager.openrc /etc/init.d/mc-manager
sudo chmod +x /etc/init.d/mc-manager

# Configurar usuario en /etc/conf.d/mc-manager (opcional)
echo 'mc_manager_user="minecraft"' | sudo tee /etc/conf.d/mc-manager

# Habilitar y arrancar
sudo rc-update add mc-manager default
sudo rc-service mc-manager start

# Ver logs
tail -f /var/log/mc-manager.log
```

### Configuración opcional (/etc/conf.d/mc-manager)
```bash
mc_manager_user="minecraft"
mc_manager_group="minecraft"
```

---

## 3. runit (Void Linux, Alpine con runit, Devuan, etc.)

### Instalación
```bash
# Crear directorio de servicio
sudo mkdir -p /etc/sv/mc-manager/log

# Copiar scripts
sudo cp runit/run /etc/sv/mc-manager/
sudo cp runit/log.run /etc/sv/mc-manager/log/run
sudo chmod +x /etc/sv/mc-manager/run /etc/sv/mc-manager/log/run

# Habilitar servicio (symlink a /var/service o /run/runit/service)
sudo ln -s /etc/sv/mc-manager /var/service/

# O en Alpine:
sudo ln -s /etc/sv/mc-manager /run/runit/service/

# Control manual
sudo sv up mc-manager      # iniciar
sudo sv down mc-manager    # parar
sudo sv restart mc-manager # reiniciar
sudo sv status mc-manager  # estado
```

### Logs
```bash
# Ver logs rotados por svlogd
tail -f /var/log/mc-manager/current
```

---

## 5. dinit (Chimera Linux, algunas distros experimentales)

### Instalación
```bash
# Copiar service file
sudo cp mc-manager.dinit /etc/dinit.d/mc-manager

# Habilitar y arrancar
sudo dinitctl enable mc-manager
sudo dinitctl start mc-manager

# Ver estado
dinitctl status mc-manager

# Ver logs
dinitctl log mc-manager
```

### Control manual
```bash
dinitctl stop mc-manager      # parar
dinitctl restart mc-manager   # reiniciar
dinitctl status mc-manager    # estado
```

### Características
- type=forking (daemon hace double-fork internamente)
- PID file automático
- restart=on-failure
- Límites NOFILE/NPROC
- Logs integrados con `dinitctl log`

---

## 6. SysV init (Legacy - Debian/Ubuntu antiguos, RHEL 6, CentOS 6, etc.)

### Instalación (Debian/Ubuntu)
```bash
sudo cp mc-manager.sysv /etc/init.d/mc-manager
sudo chmod +x /etc/init.d/mc-manager
sudo cp mc-manager.default /etc/default/mc-manager

# Ajustar usuario en /etc/default/mc-manager
sudo sed -i 's/USER="minecraft"/USER="tu_usuario"/' /etc/default/mc-manager

# Habilitar
sudo update-rc.d mc-manager defaults

# Control
sudo service mc-manager start
sudo service mc-manager stop
sudo service mc-manager status
```

### Instalación (RHEL/CentOS 6)
```bash
sudo cp mc-manager.sysv /etc/init.d/mc-manager
sudo chmod +x /etc/init.d/mc-manager
sudo cp mc-manager.default /etc/sysconfig/mc-manager

sudo chkconfig --add mc-manager
sudo chkconfig mc-manager on

sudo service mc-manager start
```

---

## Instalación del binario (requerido para todos)

```bash
# Compilar en release
cargo build --release

# Instalar binario
sudo cp target/release/Minecraft-Server-Manager-Rust /usr/local/bin/mc-manager
sudo chmod +x /usr/local/bin/mc-manager

# Verificar
mc-manager --help
```

---

## Usuario dedicado (recomendado)

```bash
# Crear usuario sin shell
sudo useradd -r -s /bin/false -d /home/minecraft -m minecraft

# Directorios de servidores
sudo -u minecraft mkdir -p /home/minecraft/minecraft-servers

# Permisos para bind ports <1024 (opcional)
sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/mc-manager
```

---

## Verificación post-instalación

```bash
# Verificar que el daemon arranca
mc-manager daemon start --foreground
# Ctrl+C para parar

# Verificar CLI
mc-manager list
mc-manager daemon status

# Verificar TUI (requiere terminal interactivo)
mc-manager tui
```

---

## Desinstalación

### systemd
```bash
sudo systemctl disable --now mc-manager
sudo rm /etc/systemd/system/mc-manager.service
sudo systemctl daemon-reload
```

### OpenRC
```bash
sudo rc-service mc-manager stop
sudo rc-update del mc-manager default
sudo rm /etc/init.d/mc-manager /etc/conf.d/mc-manager
```

### runit
```bash
sudo sv down mc-manager
sudo rm /var/service/mc-manager
sudo rm -rf /etc/sv/mc-manager
```

### SysV
```bash
sudo service mc-manager stop
sudo update-rc.d -f mc-manager remove  # Debian/Ubuntu
# o
sudo chkconfig --del mc-manager        # RHEL/CentOS
sudo rm /etc/init.d/mc-manager /etc/default/mc-manager
```

### dinit
```bash
sudo dinitctl disable mc-manager
sudo dinitctl stop mc-manager
sudo rm /etc/dinit.d/mc-manager
```

---

## Resumen de los 5 sistemas de init

| Sistema | Distros típicas | Archivo |
|---------|----------------|---------|
| **systemd** | Ubuntu 16+, Debian 8+, RHEL 7+, Fedora, Arch | `mc-manager.service` |
| **OpenRC** | Gentoo, Alpine, Artix, Devuan | `mc-manager.openrc` |
| **runit** | Void Linux, Alpine (runit), Devuan | `runit/run` + `runit/log.run` |
| **dinit** | Chimera Linux | `mc-manager.dinit` |
| **SysV init** | Debian 7-, Ubuntu 14-, RHEL 6, CentOS 6 | `mc-manager.sysv` + `mc-manager.default` |

### Instalación rápida (systemd - más común)

```bash
# 1. Instalar binario
sudo cp target/release/Minecraft-Server-Manager-Rust /usr/local/bin/mc-manager

# 2. Crear usuario
sudo useradd -r -s /bin/false -d /home/minecraft -m minecraft

# 3. Instalar service
sudo cp init/mc-manager.service /etc/systemd/system/
sudo sed -i 's/%i/minecraft/g' /etc/systemd/system/mc-manager.service

# 4. Habilitar y arrancar
sudo systemctl daemon-reload
sudo systemctl enable --now mc-manager

# 5. Verificar
mc-manager daemon status
```