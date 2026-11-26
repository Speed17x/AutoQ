//! Single-instance Unix socket IPC (for Hyprland SUPER+E / Waybar) and
//! 1-click Linux desktop integration installers (Hyprland widget, KDE Plasmoid, udev rules).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(unix)]
pub fn ipc_socket_path() -> PathBuf {
    let base = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(base).join("autoq-gui.sock")
}

#[cfg(unix)]
pub fn send_ipc_if_running(cmd: &str) -> bool {
    let path = ipc_socket_path();
    if !path.exists() {
        return false;
    }
    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            let _ = stream.write_all(cmd.as_bytes());
            true
        }
        Err(_) => {
            let _ = fs::remove_file(&path);
            false
        }
    }
}

#[cfg(not(unix))]
pub fn send_ipc_if_running(_cmd: &str) -> bool {
    false
}

#[cfg(unix)]
pub fn start_ipc_listener(tx: std::sync::mpsc::Sender<String>) {
    let path = ipc_socket_path();
    let _ = fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut s = conn;
            let mut buf = [0u8; 64];
            if let Ok(n) = s.read(&mut buf) {
                let msg = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                if !msg.is_empty() {
                    let _ = tx.send(msg);
                }
            }
        }
    });
}

#[cfg(not(unix))]
pub fn start_ipc_listener(_tx: std::sync::mpsc::Sender<String>) {}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn find_exe_sibling(name: &str) -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let cand = dir.join(name);
    if cand.is_file() {
        return Some(cand);
    }
    None
}

fn locate_bundled_dir(sub_rel: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
            if let Some(p1) = dir.parent() {
                roots.push(p1.to_path_buf());
                roots.push(p1.join("share").join("autoq"));
                if let Some(p2) = p1.parent() {
                    roots.push(p2.to_path_buf());
                }
            }
        }
    }
    roots.push(PathBuf::from("/usr/share/autoq"));
    if let Ok(cwd) = env::current_dir() {
        roots.push(cwd);
    }

    for r in roots {
        for cand in [r.join("widgets").join(sub_rel), r.join(sub_rel)] {
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    None
}

fn ensure_local_cli_installed() {
    let bin_dir = home_dir().join(".local").join("bin");
    let _ = fs::create_dir_all(&bin_dir);
    let dst = bin_dir.join("autoq-cli");
    if let Some(src) = find_exe_sibling("autoq-cli") {
        if src != dst {
            let _ = fs::copy(&src, &dst);
        }
    }
}

pub fn install_hyprland_widget() -> Result<String, String> {
    if cfg!(target_os = "windows") {
        return Err("Solo disponible en Linux.".to_string());
    }
    ensure_local_cli_installed();
    let home = home_dir();
    let bin_dir = home.join(".local/bin");
    let install_dir = home.join(".local/share/autoq-hyprland");
    let app_dir = home.join(".local/share/applications");
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&install_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

    // Copy hyprland-autoq.conf & waybar-module.jsonc if bundled, or write clean defaults
    if let Some(src) = locate_bundled_dir("linux-hyprland") {
        for f in ["hyprland-autoq.conf", "waybar-module.jsonc"] {
            let p = src.join(f);
            if p.is_file() {
                let _ = fs::copy(&p, install_dir.join(f));
            }
        }
    }

    // Point ~/.local/bin/autoq-widget to the native Rust GUI binary (or AppImage if launched from AppImage)
    let target_exec = env::var("APPIMAGE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "autoq-gui".to_string())
        });

    let wrapper = format!("#!/usr/bin/env bash\nexec \"{target_exec}\" \"$@\"\n");
    let wrapper_path = bin_dir.join("autoq-widget");
    fs::write(&wrapper_path, wrapper).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755));
    }

    let desktop = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=AutoQ\n\
         GenericName=Parametric EQ & AutoEQ\n\
         Comment=Ecualizador paramétrico universal estilo Nothing OS (Rust Nativo)\n\
         Exec={} --widget\n\
         Icon=audio-headphones\n\
         Terminal=false\n\
         Categories=AudioVideo;Audio;Mixer;\n\
         StartupWMClass=autoq-gui\n",
        wrapper_path.display()
    );
    let _ = fs::write(app_dir.join("autoq-widget.desktop"), desktop);

    Ok("Widget Hyprland + Waybar instalado en ~/.local/bin/autoq-widget.".to_string())
}

pub fn install_kde_plasmoid() -> Result<String, String> {
    if cfg!(target_os = "windows") {
        return Err("Solo disponible en Linux (KDE Plasma 6).".to_string());
    }
    ensure_local_cli_installed();
    let Some(src) = locate_bundled_dir("linux-kde/plasmoid") else {
        return Err("No se encontró la carpeta plasmoid incluida en el paquete.".to_string());
    };
    let src_s = src.to_string_lossy().into_owned();
    let up = Command::new("kpackagetool6")
        .args(["--type", "Plasma/Applet", "--upgrade", &src_s])
        .status();
    match up {
        Ok(st) if st.success() => {
            Ok("Plasmoid AutoQ actualizado en KDE Plasma 6 (motor Rust).".to_string())
        }
        Ok(_) => {
            let ins = Command::new("kpackagetool6")
                .args(["--type", "Plasma/Applet", "--install", &src_s])
                .status()
                .map_err(|e| format!("Error ejecutando kpackagetool6: {e}"))?;
            if ins.success() {
                Ok("Plasmoid AutoQ instalado en KDE Plasma 6 (motor Rust).".to_string())
            } else {
                Err("kpackagetool6 devolvió un error al instalar el Plasmoid.".to_string())
            }
        }
        Err(_) => Err("Requiere KDE Plasma 6 (kpackagetool6 no detectado).".to_string()),
    }
}

pub fn install_udev_rules() -> Result<String, String> {
    if cfg!(target_os = "windows") {
        return Err("Las reglas udev solo aplican en Linux.".to_string());
    }
    if Path::new("/etc/udev/rules.d/99-autoq-usb-dsp.rules").exists()
        || Path::new("/usr/lib/udev/rules.d/99-autoq-usb-dsp.rules").exists()
    {
        return Ok("Las reglas udev USB DSP ya están instaladas.".to_string());
    }
    let rules = concat!(
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"31b2\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"2fc6\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"2f20\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"2972\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"0a12\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"3302\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"152a\", MODE=\"0666\", TAG+=\"uaccess\"\n",
        "SUBSYSTEM==\"tty\", ATTRS{idVendor}==\"152a\", ATTRS{idProduct}==\"88fa\", MODE=\"0666\", TAG+=\"uaccess\"\n"
    );
    let tmp = env::temp_dir().join("99-autoq-usb-dsp.rules");
    fs::write(&tmp, rules).map_err(|e| e.to_string())?;
    let cmd = format!(
        "cp -f '{}' /etc/udev/rules.d/99-autoq-usb-dsp.rules && udevadm control --reload-rules && udevadm trigger",
        tmp.display()
    );
    let status = Command::new("pkexec")
        .args(["sh", "-c", &cmd])
        .status()
        .map_err(|e| format!("pkexec no disponible: {e}"))?;
    let _ = fs::remove_file(&tmp);
    if status.success() {
        Ok("Permisos USB DSP activados (reglas udev recargadas).".to_string())
    } else {
        Err("Instalación de reglas udev cancelada o fallida.".to_string())
    }
}

pub fn is_daemon_service_active() -> bool {
    if cfg!(target_os = "windows") {
        return false;
    }
    Command::new("systemctl")
        .args(["--user", "is-active", "autoq-daemon.service"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false)
}

pub fn install_daemon_service() -> Result<String, String> {
    if cfg!(target_os = "windows") {
        return Err("Solo disponible en Linux (systemd user service).".to_string());
    }
    let home = home_dir();
    let svc_dir = home.join(".config/systemd/user");
    fs::create_dir_all(&svc_dir).map_err(|e| e.to_string())?;

    let exec_line = if let Some(appimage) = env::var("APPIMAGE").ok().filter(|s| !s.is_empty()) {
        format!("\"{appimage}\" --daemon")
    } else if let Some(daemon_bin) = find_exe_sibling("autoq-daemon") {
        format!("\"{}\"", daemon_bin.display())
    } else {
        "autoq-daemon".to_string()
    };

    let unit = format!(
        "[Unit]\n\
         Description=AutoQ Background Auto-Profile Daemon (udev + PipeWire)\n\
         After=pipewire.service pipewire-pulse.service\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exec_line}\n\
         Restart=on-failure\n\
         RestartSec=3\n\n\
         [Install]\n\
         WantedBy=default.target\n"
    );
    let unit_path = svc_dir.join("autoq-daemon.service");
    fs::write(&unit_path, unit).map_err(|e| e.to_string())?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    let st = Command::new("systemctl")
        .args(["--user", "enable", "--now", "autoq-daemon.service"])
        .status()
        .map_err(|e| format!("systemctl --user falló: {e}"))?;
    if st.success() {
        Ok("Daemon autoq-daemon activado en segundo plano (systemd --user).".to_string())
    } else {
        Err("systemctl --user devolvió error al activar autoq-daemon.".to_string())
    }
}

pub fn uninstall_daemon_service() -> Result<String, String> {
    if cfg!(target_os = "windows") {
        return Err("Solo disponible en Linux.".to_string());
    }
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "autoq-daemon.service"])
        .status();
    let unit_path = home_dir().join(".config/systemd/user/autoq-daemon.service");
    let _ = fs::remove_file(&unit_path);
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    Ok("Daemon autoq-daemon detenido y deshabilitado.".to_string())
}
