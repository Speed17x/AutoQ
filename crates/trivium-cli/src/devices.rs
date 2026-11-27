//! Device discovery, hardware scans, and fast telemetry commands for `trivium-cli`.

use crate::eq_cmd::entry_to_json;
use serde_json::{json, Value};
use std::collections::HashSet;
use trivium_core::autoeq::{
    downsample_curve, find_best_entry_any_brand, load_catalog, reconstruct_fr_curve, CURATED_TARGETS,
    SQUIG_SOURCES,
};
use trivium_core::biquad::{fit_band_count, flat_bands};
use trivium_core::config::{load_presets, load_ui_settings, DEFAULT_VIRTUAL_BANDS};
use trivium_core::hid::{self, MoondropDeviceInfo};
use trivium_core::platform_linux::{self, PipeWireAudioDevice, PW_SINK_NAME};
use trivium_core::platform_windows;

#[derive(Clone)]
pub(crate) enum RuntimeDevice {
    UsbHid(MoondropDeviceInfo),
    PipeWire(PipeWireAudioDevice),
    WinApo { apo_ready: bool },
}

impl RuntimeDevice {
    pub(crate) fn id(&self) -> String {
        match self {
            Self::UsbHid(d) => format!("usb:{}", d.hidraw_path),
            Self::PipeWire(p) => format!("pw:{}", p.sink_name),
            Self::WinApo { .. } => "win:equalizer_apo".to_string(),
        }
    }

    pub(crate) fn display_name(&self) -> String {
        match self {
            Self::UsbHid(d) => d.display_name.clone(),
            Self::PipeWire(p) => p.display_name.clone(),
            Self::WinApo { apo_ready } => {
                if *apo_ready {
                    "Windows Global (Equalizer APO)".to_string()
                } else {
                    "Equalizer APO (no detectado)".to_string()
                }
            }
        }
    }

    pub(crate) fn transport_label(&self) -> String {
        match self {
            Self::UsbHid(d) => d.protocol_badge(),
            Self::PipeWire(p) => p.transport_badge.clone(),
            Self::WinApo { apo_ready } => {
                if *apo_ready {
                    "APO·10B".to_string()
                } else {
                    "APO·OFF".to_string()
                }
            }
        }
    }

    pub(crate) fn max_bands(&self) -> usize {
        match self {
            Self::UsbHid(d) => d.max_bands,
            Self::PipeWire(p) => p.max_bands,
            Self::WinApo { .. } => DEFAULT_VIRTUAL_BANDS,
        }
    }

    pub(crate) fn autoeq_hint(&self) -> &str {
        match self {
            Self::UsbHid(d) => &d.autoeq_hint,
            Self::PipeWire(p) => &p.autoeq_hint,
            Self::WinApo { .. } => "",
        }
    }

    pub(crate) fn to_public_json(&self) -> Value {
        match self {
            Self::UsbHid(d) => json!({
                "id": self.id(),
                "kind": "usb_hid",
                "transport": "usb_hid",
                "display_name": d.display_name,
                "transport_label": self.transport_label(),
                "protocol": d.protocol,
                "max_bands": d.max_bands,
                "max_gain_db": d.max_gain_db,
                "supports_shelves": d.supports_shelves,
                "supports_spp": false,
                "disconnects_on_commit": d.disconnects_on_commit,
                "autoeq_hint": d.autoeq_hint,
                "hidraw_path": d.hidraw_path,
                "vid_pid": d.vid_pid(),
            }),
            Self::PipeWire(p) => json!({
                "id": self.id(),
                "kind": p.kind,
                "transport": p.kind,
                "display_name": p.display_name,
                "transport_label": p.transport_badge,
                "max_bands": p.max_bands,
                "max_gain_db": p.max_gain_db,
                "supports_shelves": p.supports_shelves,
                "supports_spp": p.supports_spp,
                "autoeq_hint": p.autoeq_hint,
                "sink_name": p.sink_name,
                "mac": p.mac,
            }),
            Self::WinApo { apo_ready } => json!({
                "id": self.id(),
                "kind": "win_apo",
                "transport": "win_apo",
                "display_name": self.display_name(),
                "transport_label": self.transport_label(),
                "protocol": "equalizer_apo",
                "max_bands": DEFAULT_VIRTUAL_BANDS,
                "max_gain_db": 15.0,
                "supports_shelves": true,
                "supports_spp": false,
                "disconnects_on_commit": false,
                "autoeq_hint": "",
                "apo_ready": apo_ready,
            }),
        }
    }
}

pub(crate) fn discover_all_devices() -> Vec<RuntimeDevice> {
    let mut out = Vec::new();
    let usb_objs = hid::scan_moondrop_devices();
    let hid_vid_pids: Vec<String> = usb_objs.iter().map(MoondropDeviceInfo::vid_pid).collect();

    for d in usb_objs {
        out.push(RuntimeDevice::UsbHid(d));
    }

    if cfg!(target_os = "windows") {
        let apo_ready = platform_windows::find_apo_config_dir().is_some();
        out.push(RuntimeDevice::WinApo { apo_ready });
    } else {
        for p in platform_linux::scan_pipewire_audio_sinks(&hid_vid_pids) {
            out.push(RuntimeDevice::PipeWire(p));
        }
    }
    out
}

pub(crate) fn pick_device<'a>(
    devices: &'a [RuntimeDevice],
    preferred_id: &str,
) -> Option<&'a RuntimeDevice> {
    if devices.is_empty() {
        return None;
    }
    if !preferred_id.is_empty() {
        if let Some(d) = devices.iter().find(|d| d.id() == preferred_id) {
            return Some(d);
        }
    }
    if cfg!(target_os = "linux") {
        if let Some(ds) = platform_linux::get_default_sink() {
            for d in devices {
                if let RuntimeDevice::PipeWire(p) = d {
                    if p.sink_name == ds || ds == PW_SINK_NAME {
                        return Some(d);
                    }
                }
            }
        }
    }
    devices.first()
}

pub(crate) fn cmd_status(preferred_id: &str, refresh_db: bool) -> Value {
    let devices = discover_all_devices();
    let active = pick_device(&devices, preferred_id);

    let mut bands = flat_bands(
        active
            .map(RuntimeDevice::max_bands)
            .unwrap_or(DEFAULT_VIRTUAL_BANDS),
    );
    let mut dsp_enabled = true;
    let mut read_error: Option<String> = None;
    let pw_active = platform_linux::is_pipewire_eq_running();

    if let Some(dev) = active {
        match dev {
            RuntimeDevice::UsbHid(info) => match hid::read_device_eq(Some(info)) {
                Ok((_, en, b)) => {
                    dsp_enabled = en;
                    bands = b;
                }
                Err(e) => {
                    read_error = Some(e);
                    bands = flat_bands(info.max_bands);
                }
            },
            RuntimeDevice::PipeWire(pw) => {
                bands = platform_linux::load_pw_saved_bands(pw);
                dsp_enabled = pw_active;
            }
            RuntimeDevice::WinApo { .. } => {
                let st = platform_windows::load_win_virtual_state();
                dsp_enabled = st.enabled;
                bands = fit_band_count(&st.bands, DEFAULT_VIRTUAL_BANDS);
            }
        }
    }

    let (entries, active_sources, total_sources) = load_catalog(refresh_db);
    let hint = active.map(RuntimeDevice::autoeq_hint).unwrap_or("");
    let auto_src_entry = if !hint.is_empty() {
        find_best_entry_any_brand(&entries, hint)
    } else {
        None
    };

    let stock_fr_curve = auto_src_entry
        .and_then(|e| reconstruct_fr_curve(e).ok())
        .map(|(f, v)| downsample_curve(&f, &v, 64))
        .unwrap_or_default();

    let mut quick_sources: Vec<Value> = Vec::with_capacity(80);
    let mut seen_keys = HashSet::new();
    if let Some(ae) = auto_src_entry {
        seen_keys.insert(ae.unique_key());
        quick_sources.push(entry_to_json(ae));
    }
    for e in &entries {
        if quick_sources.len() >= 80 {
            break;
        }
        if seen_keys.insert(e.unique_key()) {
            quick_sources.push(entry_to_json(e));
        }
    }

    let targets_json: Vec<Value> = CURATED_TARGETS
        .iter()
        .map(|&(label, file)| json!({ "label": label, "file": file }))
        .collect();

    json!({
        "ok": true,
        "engine": "rust",
        "device": active.map(RuntimeDevice::to_public_json),
        "devices": devices.iter().map(RuntimeDevice::to_public_json).collect::<Vec<_>>(),
        "bands": bands,
        "dsp_enabled": dsp_enabled,
        "pipewire_active": pw_active,
        "read_error": read_error,
        "presets": load_presets(),
        "targets": targets_json,
        "moondrop_models": quick_sources,
        "auto_source_index": 0,
        "auto_source_entry": auto_src_entry.map(entry_to_json),
        "stock_fr_curve": stock_fr_curve,
        "total_db_count": entries.len(),
        "active_sources": active_sources,
        "total_sources": total_sources,
        "ui_settings": load_ui_settings().resolved_palette_json(),
        "db_error": Value::Null,
    })
}

pub(crate) fn cmd_waybar_status() -> Value {
    let devices = discover_all_devices();
    let active = pick_device(&devices, "");

    let Some(dev) = active else {
        return json!({
            "text": "○ TRIVIUM",
            "tooltip": "Trivium — Sin dispositivo",
            "class": "offline",
        });
    };

    let on = match dev {
        RuntimeDevice::UsbHid(info) => hid::read_device_eq(Some(info))
            .map(|(_, en, _)| en)
            .unwrap_or(true),
        RuntimeDevice::PipeWire(_) => platform_linux::is_pipewire_eq_running(),
        RuntimeDevice::WinApo { .. } => platform_windows::load_win_virtual_state().enabled,
    };

    let cfile = trivium_core::config::cache_dir().join("catalog_v2.json");
    let cached_count = std::fs::read_to_string(cfile)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok())
        .map(|v| v.len())
        .unwrap_or(0);

    let badge = dev.transport_label();
    let dot = if on { "●" } else { "○" };
    let db_line = if cached_count > 0 {
        format!("{cached_count} curvas ({} fuentes)", 1 + SQUIG_SOURCES.len())
    } else {
        "Catálogo AutoEQ listo para sincronizar".to_string()
    };

    json!({
        "text": format!("{dot} {badge}"),
        "tooltip": format!("Trivium — {}\n{}\nClick: abrir widget flotante", dev.display_name(), db_line),
        "class": if on { "active" } else { "bypass" },
    })
}

pub(crate) fn handle_fast_device_cmd(raw: &[String]) -> bool {
    match raw[0].as_str() {
        "poll" => {
            println!("{}", hid::poll_fingerprint());
            true
        }
        "ble-protocols" => {
            print_ble_protocols(raw);
            true
        }
        "hid-scan" => {
            print_hid_scan(raw);
            true
        }
        _ => false,
    }
}

fn print_ble_protocols(raw: &[String]) {
    let profiles = trivium_core::ble_dsp::supported_ble_profiles();
    if raw.iter().any(|a| a == "--json") {
        println!(
            "{}",
            serde_json::to_string(&json!({ "ok": true, "profiles": profiles })).unwrap_or_default()
        );
        return;
    }
    for p in profiles {
        println!(
            "- {} | service={} | tx={} | rx={} | {}B (±{:.0} dB, {} slot(s))",
            p.display_name,
            p.service_uuid,
            p.tx_char_uuid,
            p.rx_char_uuid,
            p.max_bands,
            p.max_gain_db,
            p.slots
        );
    }
}

fn print_hid_scan(raw: &[String]) {
    let dump_desc = raw.iter().any(|a| a == "--dump-descriptors" || a == "-d");
    let show_all = raw.iter().any(|a| a == "--all" || dump_desc);
    let as_json = raw.iter().any(|a| a == "--json");
    let all = hid::scan_all_hid_devices_detailed(dump_desc);
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|d| show_all || d.recognized)
        .collect();

    if as_json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "ok": true,
                "devices": filtered,
                "report_hint": "Comparte este JSON en https://github.com/Speed17x/Trivium/issues para añadir soporte a tu DAC/cable DSP"
            }))
            .unwrap_or_default()
        );
        return;
    }

    if filtered.is_empty() {
        println!("No se detectaron dispositivos DSP HID compatibles. Usa `trivium-cli hid-scan --all --dump-descriptors` para ver todos los nodos HID.");
        return;
    }

    println!(
        "{:<16} {:<10} {:<12} {:<26} NOMBRE HID",
        "RUTA", "VID:PID", "PROTOCOLO", "MODELO DETECTADO"
    );
    println!("{}", "-".repeat(86));
    for d in &filtered {
        let proto = d
            .protocol
            .map(|p| format!("{p:?}"))
            .unwrap_or_else(|| "desconocido".to_string());
        let matched = d.matched_name.as_deref().unwrap_or("-");
        println!(
            "{:<16} {:<10} {:<12} {:<26} {}",
            d.path, d.vid_pid, proto, matched, d.hid_name
        );
        if dump_desc {
            let up = d
                .usage_page
                .map(|v| format!("0x{v:04X}"))
                .unwrap_or_else(|| "-".to_string());
            let u = d
                .usage
                .map(|v| format!("0x{v:02X}"))
                .unwrap_or_else(|| "-".to_string());
            let dlen = d
                .report_descriptor_len
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".to_string());
            println!("    UsagePage={up}  Usage={u}  DescLen={dlen}B");
            if let Some(hex) = &d.report_descriptor_hex {
                println!("    DescHex={hex}");
            }
        }
    }
    if dump_desc {
        println!("\nPara reportar hardware nuevo: `trivium-cli hid-scan --dump-descriptors --json` y abre un issue en https://github.com/Speed17x/Trivium/issues");
    }
}
