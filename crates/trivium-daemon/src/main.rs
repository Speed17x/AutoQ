//! Optional background auto-profile daemon for Trivium (`trivium-daemon`).
//!
//! Listens to Linux `udevadm monitor` + `pactl subscribe` events (or lightweight
//! cross-platform polling on Windows) and automatically applies saved EQ profiles
//! when a matching USB-C DSP, USB-CDC DAC, Bluetooth headset, or PipeWire sink is connected.

use trivium_core::config::{
    bt_profiles_file, daemon_profiles_file, load_daemon_config, load_presets,
    load_saved_profile_bands, match_rule_for_device, resolve_rule_bands, DaemonConfig,
};
use trivium_core::hid::{self, MoondropDeviceInfo};
use trivium_core::platform_linux::{self, PipeWireAudioDevice, PW_SINK_NAME};
use serde_json::json;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Default)]
struct DaemonState {
    applied_usb: HashSet<String>,
    last_pw_sink: String,
    recent_log: VecDeque<String>,
}

impl DaemonState {
    fn log_line(&mut self, msg: impl Into<String>) {
        let s = msg.into();
        eprintln!("[trivium-daemon] {s}");
        if self.recent_log.len() >= 30 {
            self.recent_log.pop_front();
        }
        self.recent_log.push_back(s);
    }
}

fn usb_device_key(d: &MoondropDeviceInfo) -> String {
    format!("{}:{}:{}", d.hidraw_path, d.vid_pid(), d.display_name)
}

fn reconcile_once(state: &mut DaemonState, dry_run: bool) {
    let cfg: DaemonConfig = load_daemon_config();
    if !cfg.enabled {
        return;
    }
    let presets = load_presets();

    // 1. Reconcile newly plugged USB HID / USB-CDC DSP devices
    let usb_devs = hid::scan_moondrop_devices();
    let current_usb_keys: HashSet<String> = usb_devs.iter().map(usb_device_key).collect();
    state.applied_usb.retain(|k| current_usb_keys.contains(k));

    for dev in &usb_devs {
        let key = usb_device_key(dev);
        if state.applied_usb.contains(&key) {
            continue;
        }
        state.applied_usb.insert(key);
        let id = format!("usb:{}", dev.hidraw_path);
        if let Some(rule) = match_rule_for_device(&cfg, &id, &dev.display_name, &dev.vid_pid()) {
            let bands = resolve_rule_bands(rule, &presets);
            if bands.is_empty() {
                continue;
            }
            if dry_run {
                state.log_line(format!(
                    "[dry-run] Aplicaría regla '{}' -> {} ({})",
                    rule.match_pattern, dev.display_name, dev.hidraw_path
                ));
            } else {
                match hid::write_device_eq(&bands, rule.commit_flash, rule.enable_eq, Some(dev)) {
                    Ok(_) => state.log_line(format!(
                        "Perfil aplicado a '{}' ({}, {} bandas)",
                        dev.display_name,
                        dev.protocol_badge(),
                        bands.len().min(dev.max_bands)
                    )),
                    Err(e) => state.log_line(format!(
                        "Error aplicando perfil a '{}': {e}",
                        dev.display_name
                    )),
                }
            }
        }
    }

    // 2. Reconcile PipeWire / Bluetooth default sink changes (Linux only)
    #[cfg(target_os = "linux")]
    {
        let Some(default_sink) = platform_linux::get_default_sink() else {
            return;
        };
        if default_sink == PW_SINK_NAME
            || default_sink.contains("auto_null")
            || default_sink.to_ascii_lowercase().contains("dummy")
        {
            return;
        }
        if default_sink == state.last_pw_sink {
            return;
        }
        state.last_pw_sink = default_sink.clone();

        let hid_vps: Vec<String> = usb_devs.iter().map(MoondropDeviceInfo::vid_pid).collect();
        let sinks = platform_linux::scan_pipewire_audio_sinks(&hid_vps);
        let Some(pw_dev): Option<&PipeWireAudioDevice> =
            sinks.iter().find(|p| p.sink_name == default_sink)
        else {
            return;
        };

        let id = format!("pw:{}", pw_dev.sink_name);
        if let Some(rule) = match_rule_for_device(&cfg, &id, &pw_dev.display_name, &pw_dev.mac) {
            let bands = resolve_rule_bands(rule, &presets);
            if !bands.is_empty() {
                if dry_run {
                    state.log_line(format!(
                        "[dry-run] Aplicaría regla '{}' a PipeWire sink '{}'",
                        rule.match_pattern, pw_dev.display_name
                    ));
                } else {
                    match platform_linux::apply_pipewire_eq(pw_dev, &bands, rule.enable_eq) {
                        Ok(active) => state.log_line(format!(
                            "PipeWire EQ {} para '{}' ({})",
                            if active { "activo" } else { "en bypass" },
                            pw_dev.display_name,
                            rule.match_pattern
                        )),
                        Err(e) => state.log_line(format!(
                            "Error aplicando PipeWire EQ a '{}': {e}",
                            pw_dev.display_name
                        )),
                    }
                }
            }
            return;
        }

        if cfg.auto_apply_saved_pipewire {
            let saved = load_saved_profile_bands(
                &bt_profiles_file(),
                if !pw_dev.mac.is_empty() {
                    &pw_dev.mac
                } else {
                    &pw_dev.sink_name
                },
                pw_dev.max_bands,
            );
            let has_non_flat = saved.iter().any(|b| b.gain.abs() > 0.05);
            if has_non_flat {
                if dry_run {
                    state.log_line(format!(
                        "[dry-run] Aplicaría perfil guardado para '{}'",
                        pw_dev.display_name
                    ));
                } else if let Ok(active) = platform_linux::apply_pipewire_eq(pw_dev, &saved, true) {
                    state.log_line(format!(
                        "Perfil guardado restaurado en PipeWire ({}) -> '{}'",
                        if active { "activo" } else { "bypass" },
                        pw_dev.display_name
                    ));
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_udev_watcher(tx: Sender<()>) {
    thread::spawn(move || loop {
        let Ok(mut child) = Command::new("udevadm")
            .args([
                "monitor",
                "--udev",
                "--subsystem-match=hidraw",
                "--subsystem-match=tty",
                "--subsystem-match=bluetooth",
                "--subsystem-match=sound",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if line.contains("add") || line.contains("remove") || line.contains("bind") {
                    let _ = tx.send(());
                }
            }
        }
        let _ = child.wait();
        thread::sleep(Duration::from_secs(2));
    });
}

#[cfg(target_os = "linux")]
fn spawn_pipewire_watcher(tx: Sender<()>) {
    thread::spawn(move || loop {
        let Ok(mut child) = Command::new("pactl")
            .args(["subscribe"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let low = line.to_ascii_lowercase();
                if low.contains("sink") || low.contains("server") {
                    let _ = tx.send(());
                }
            }
        }
        let _ = child.wait();
        thread::sleep(Duration::from_secs(2));
    });
}

#[cfg(target_os = "linux")]
fn install_systemd_user_service() -> Result<String, String> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let svc_dir = PathBuf::from(&home).join(".config/systemd/user");
    fs::create_dir_all(&svc_dir).map_err(|e| e.to_string())?;
    let exe = env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "trivium-daemon".to_string());
    let unit = format!(
        "[Unit]\n\
         Description=Trivium Background Auto-Profile Daemon\n\
         After=pipewire.service pipewire-pulse.service\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exe}\n\
         Restart=on-failure\n\
         RestartSec=3\n\n\
         [Install]\n\
         WantedBy=default.target\n"
    );
    let unit_path = svc_dir.join("trivium-daemon.service");
    fs::write(&unit_path, unit).map_err(|e| e.to_string())?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    let st = Command::new("systemctl")
        .args(["--user", "enable", "--now", "trivium-daemon.service"])
        .status()
        .map_err(|e| format!("systemctl --user falló: {e}"))?;
    if st.success() {
        Ok(format!(
            "Servicio de usuario habilitado y activo en {}",
            unit_path.display()
        ))
    } else {
        Err("systemctl --user enable --now devolvió un error.".to_string())
    }
}

#[cfg(target_os = "linux")]
fn uninstall_systemd_user_service() -> Result<String, String> {
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "trivium-daemon.service"])
        .status();
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let unit_path = PathBuf::from(&home).join(".config/systemd/user/trivium-daemon.service");
    let _ = fs::remove_file(&unit_path);
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    Ok("Servicio trivium-daemon.service desinstalado.".to_string())
}

#[cfg(not(target_os = "linux"))]
fn install_systemd_user_service() -> Result<String, String> {
    Err("La instalación de servicio systemd solo aplica en Linux.".to_string())
}

#[cfg(not(target_os = "linux"))]
fn uninstall_systemd_user_service() -> Result<String, String> {
    Err("La desinstalación de servicio systemd solo aplica en Linux.".to_string())
}

fn print_status() {
    let cfg = load_daemon_config();
    let active_systemd = if cfg!(target_os = "linux") {
        Command::new("systemctl")
            .args(["--user", "is-active", "trivium-daemon.service"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
            .unwrap_or(false)
    } else {
        false
    };
    let usb_devs = hid::scan_moondrop_devices();
    println!(
        "{}",
        json!({
            "ok": true,
            "version": env!("CARGO_PKG_VERSION"),
            "config_file": daemon_profiles_file(),
            "enabled": cfg.enabled,
            "poll_interval_ms": cfg.poll_interval_ms,
            "auto_apply_saved_pipewire": cfg.auto_apply_saved_pipewire,
            "rules_count": cfg.rules.len(),
            "systemd_active": active_systemd,
            "usb_devices": usb_devs.iter().map(|d| json!({
                "name": d.display_name,
                "vid_pid": d.vid_pid(),
                "protocol": d.protocol_badge(),
                "path": d.hidraw_path,
            })).collect::<Vec<_>>(),
        })
    );
}

fn print_help() {
    println!(
        "trivium-daemon {} — Servicio residente opcional de auto-perfil para Trivium\n\n\
         Uso:\n\
           trivium-daemon                    Inicia el bucle residente (udev + PipeWire / sondeo)\n\
           trivium-daemon --once [--dry-run] Ejecuta una única pasada de reconciliación y sale\n\
           trivium-daemon --status           Muestra estado JSON, reglas y dispositivos detectados\n\
           trivium-daemon --install-service  Instala y activa ~/.config/systemd/user/trivium-daemon.service\n\
           trivium-daemon --uninstall-service Detiene y elimina el servicio systemd de usuario\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help" || a == "help") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("trivium-daemon {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--status") {
        print_status();
        return;
    }
    if args.iter().any(|a| a == "--install-service") {
        match install_systemd_user_service() {
            Ok(m) => println!("[OK] {m}"),
            Err(e) => {
                eprintln!("[ERROR] {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--uninstall-service") {
        match uninstall_systemd_user_service() {
            Ok(m) => println!("[OK] {m}"),
            Err(e) => {
                eprintln!("[ERROR] {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let once = args.iter().any(|a| a == "--once");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let mut state = DaemonState::default();
    reconcile_once(&mut state, dry_run);
    if once {
        return;
    }

    let (tx, rx): (Sender<()>, Receiver<()>) = mpsc::channel();
    #[cfg(target_os = "linux")]
    {
        spawn_udev_watcher(tx.clone());
        spawn_pipewire_watcher(tx);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = tx;

    let mut last_fp = hid::poll_fingerprint();
    let mut last_pw = platform_linux::get_default_sink().unwrap_or_default();

    loop {
        let interval = Duration::from_millis(load_daemon_config().poll_interval_ms.clamp(500, 10_000));
        match rx.recv_timeout(interval) {
            Ok(()) => {
                // Debounce rapid bursts of udev/PipeWire events (450 ms)
                let settle_until = Instant::now() + Duration::from_millis(450);
                while Instant::now() < settle_until {
                    let rem = settle_until.saturating_duration_since(Instant::now());
                    if rx.recv_timeout(rem).is_err() {
                        break;
                    }
                }
                reconcile_once(&mut state, dry_run);
                last_fp = hid::poll_fingerprint();
                last_pw = platform_linux::get_default_sink().unwrap_or_default();
            }
            Err(_) => {
                let fp = hid::poll_fingerprint();
                let pw = platform_linux::get_default_sink().unwrap_or_default();
                if fp != last_fp || pw != last_pw {
                    last_fp = fp;
                    last_pw = pw;
                    reconcile_once(&mut state, dry_run);
                }
            }
        }
    }
}
