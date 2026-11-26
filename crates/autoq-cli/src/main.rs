//! Unified AutoQ JSON CLI & ultra-fast telemetry binary (`autoq-cli`).
//!
//! Drop-in compatible with `moondrop_backend.py` (Linux/Hyprland/KDE) and
//! `moondrop_backend_win.py` (Windows), plus native sub-millisecond `poll` and
//! `waybar-status` subcommands.

use autoq_core::autoeq::{
    compute_match_headphone_peq, compute_target_curve_peq, downsample_curve, export_parametric_eq,
    find_best_entry_any_brand, load_catalog, normalize_search_text, parse_parametric_eq,
    reconstruct_fr_curve, AutoEqEntry, CURATED_TARGETS, SQUIG_SOURCES,
};
use autoq_core::biquad::{fit_band_count, flat_bands, FilterBand};
use autoq_core::config::{load_presets, save_presets, DEFAULT_VIRTUAL_BANDS};
use autoq_core::hid::{self, MoondropDeviceInfo};
use autoq_core::platform_linux::{self, PipeWireAudioDevice, PW_SINK_NAME};
use autoq_core::platform_windows;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;

#[derive(Clone)]
enum RuntimeDevice {
    UsbHid(MoondropDeviceInfo),
    PipeWire(PipeWireAudioDevice),
    WinApo { apo_ready: bool },
}

impl RuntimeDevice {
    fn id(&self) -> String {
        match self {
            Self::UsbHid(d) => format!("usb:{}", d.hidraw_path),
            Self::PipeWire(p) => format!("pw:{}", p.sink_name),
            Self::WinApo { .. } => "win:equalizer_apo".to_string(),
        }
    }

    fn display_name(&self) -> String {
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

    fn transport_label(&self) -> String {
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

    fn max_bands(&self) -> usize {
        match self {
            Self::UsbHid(d) => d.max_bands,
            Self::PipeWire(p) => p.max_bands,
            Self::WinApo { .. } => DEFAULT_VIRTUAL_BANDS,
        }
    }

    fn autoeq_hint(&self) -> &str {
        match self {
            Self::UsbHid(d) => &d.autoeq_hint,
            Self::PipeWire(p) => &p.autoeq_hint,
            Self::WinApo { .. } => "",
        }
    }

    fn to_public_json(&self) -> Value {
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

fn discover_all_devices() -> Vec<RuntimeDevice> {
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

fn pick_device<'a>(devices: &'a [RuntimeDevice], preferred_id: &str) -> Option<&'a RuntimeDevice> {
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

fn entry_to_json(e: &AutoEqEntry) -> Value {
    json!({
        "name": e.name,
        "source": e.source_rig,
        "source_rig": e.source_rig,
        "source_kind": e.source_kind,
        "rel_path": e.unique_key(),
        "raw_rel_path": e.rel_path,
        "form_factor": e.form_factor(),
        "label": format!("{} [{}]", e.name, e.source_rig),
    })
}

fn find_entry_by_key<'a>(entries: &'a [AutoEqEntry], key: &str) -> Option<&'a AutoEqEntry> {
    let trimmed = key.trim();
    entries
        .iter()
        .find(|e| e.unique_key() == trimmed || e.rel_path == trimmed)
}

fn decode_b64(val: &str) -> Result<String, String> {
    if val.is_empty() {
        return Ok(String::new());
    }
    let bytes = B64
        .decode(val.trim().as_bytes())
        .map_err(|e| format!("Base64 inválido: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 inválido en Base64: {e}"))
}

fn parse_bands_json(raw: &str) -> Result<Vec<FilterBand>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<FilterBand> =
        serde_json::from_str(raw).map_err(|e| format!("JSON de bandas inválido: {e}"))?;
    Ok(items.into_iter().map(|b| b.clamped()).collect())
}

fn resolve_target_csv_arg(raw_target: &str) -> Option<String> {
    let t = raw_target.trim();
    if t.is_empty() {
        return None;
    }
    // Accept either the CSV filename (`Harman in-ear 2019.csv`) or the display label (`Harman In-Ear 2019`)
    for &(label, file) in CURATED_TARGETS {
        if t.eq_ignore_ascii_case(label) {
            return if file.is_empty() {
                None
            } else {
                Some(file.to_string())
            };
        }
    }
    Some(t.to_string())
}

fn cmd_status(preferred_id: &str, refresh_db: bool) -> Value {
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
        "db_error": Value::Null,
    })
}

fn cmd_waybar_status() -> Value {
    let devices = discover_all_devices();
    let active = pick_device(&devices, "");

    let Some(dev) = active else {
        return json!({
            "text": "○ AUTOQ",
            "tooltip": "AutoQ (Rust) — Sin dispositivo",
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

    // Fast cached count without triggering network sync
    let cfile = autoq_core::config::cache_dir().join("catalog_v2.json");
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
        "tooltip": format!("AutoQ — {}\n{}\nClick: abrir widget Nothing OS", dev.display_name(), db_line),
        "class": if on { "active" } else { "bypass" },
    })
}

fn cmd_apply(
    device_id: &str,
    bands_json: &str,
    commit: bool,
    enable_eq: bool,
) -> Result<Value, String> {
    let bands = parse_bands_json(bands_json)?;
    let devices = discover_all_devices();
    let Some(dev) = pick_device(&devices, device_id) else {
        return Ok(json!({
            "ok": false,
            "error": "No hay ningún dispositivo de audio conectado."
        }));
    };

    let fitted = fit_band_count(&bands, dev.max_bands());
    match dev {
        RuntimeDevice::UsbHid(info) => {
            hid::write_device_eq(&fitted, commit, enable_eq, Some(info))?;
            let msg = if commit {
                format!("Guardado en flash ({}).", info.display_name)
            } else if enable_eq {
                format!("DSP activo ({}).", info.display_name)
            } else {
                format!("DSP en bypass ({}).", info.display_name)
            };
            Ok(json!({
                "ok": true,
                "kind": "usb_hid",
                "dsp_enabled": enable_eq,
                "committed": commit,
                "bands": fitted,
                "message": msg,
            }))
        }
        RuntimeDevice::PipeWire(pw) => {
            let active = platform_linux::apply_pipewire_eq(pw, &fitted, enable_eq)?;
            let msg = if active {
                format!("EQ PipeWire {}B activo para {}.", pw.max_bands, pw.display_name)
            } else {
                format!("EQ en bypass/plano para {}.", pw.display_name)
            };
            Ok(json!({
                "ok": true,
                "kind": pw.kind,
                "dsp_enabled": active,
                "pipewire_active": active,
                "bands": fitted,
                "message": msg,
            }))
        }
        RuntimeDevice::WinApo { .. } => {
            let path = platform_windows::apply_apo_eq(&fitted, enable_eq)?;
            let fname = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "autoq_eq.txt".to_string());
            let msg = if enable_eq {
                format!("EQ aplicado en Equalizer APO ({fname}).")
            } else {
                "Bypass aplicado en Equalizer APO.".to_string()
            };
            Ok(json!({
                "ok": true,
                "kind": "win_apo",
                "dsp_enabled": enable_eq,
                "bands": fitted,
                "message": msg,
            }))
        }
    }
}

fn cmd_flat(device_id: &str, commit: bool) -> Result<Value, String> {
    let devices = discover_all_devices();
    let Some(dev) = pick_device(&devices, device_id) else {
        platform_linux::stop_pipewire_eq(None);
        let bands = flat_bands(DEFAULT_VIRTUAL_BANDS);
        return Ok(json!({
            "ok": true,
            "bands": bands,
            "pipewire_active": false,
            "message": "EQ en plano (0 dB)."
        }));
    };
    let bands = flat_bands(dev.max_bands());
    let raw = serde_json::to_string(&bands).map_err(|e| e.to_string())?;
    cmd_apply(&dev.id(), &raw, commit, true)
}

fn cmd_search(query: &str, moondrop_only: bool, in_ear_only: bool, limit: usize) -> Value {
    let (entries, _, _) = load_catalog(false);
    let norm_q = normalize_search_text(query);
    let tokens: Vec<&str> = norm_q.split_whitespace().filter(|t| !t.is_empty()).collect();

    let mut matches: Vec<&AutoEqEntry> = entries
        .iter()
        .filter(|e| {
            if in_ear_only && e.form_factor() != "in-ear" {
                return false;
            }
            if moondrop_only && !e.name.to_ascii_lowercase().contains("moondrop") {
                return false;
            }
            let hay = normalize_search_text(&format!("{} {}", e.name, e.source_rig));
            tokens.iter().all(|tok| hay.contains(tok))
        })
        .collect();

    matches.sort_by(|a, b| {
        let rank_entry = |e: &AutoEqEntry| {
            let s = e.source_rig.to_ascii_lowercase();
            let name_norm = normalize_search_text(&e.name);
            let exact_boost = if !norm_q.is_empty() && name_norm.contains(&norm_q) {
                0u8
            } else {
                1u8
            };
            let r = if s.contains("super* review") {
                0u8
            } else if s.contains("crinacle on 711") {
                1u8
            } else if s.contains("regancipher") {
                2u8
            } else if s.contains("oratory1990") {
                3u8
            } else if s.contains("aftersound") || s.contains("hbb") {
                4u8
            } else if e.form_factor() == "in-ear" {
                5u8
            } else {
                6u8
            };
            (exact_boost, r, e.name.len(), e.name.to_lowercase())
        };
        rank_entry(a).cmp(&rank_entry(b))
    });

    let total_matches = matches.len();
    let results: Vec<Value> = matches
        .into_iter()
        .take(limit)
        .map(entry_to_json)
        .collect();

    json!({
        "ok": true,
        "total_matches": total_matches,
        "results": results,
    })
}

fn cmd_source_fr(src_key: &str) -> Result<Value, String> {
    let (entries, _, _) = load_catalog(false);
    let Some(src) = find_entry_by_key(&entries, src_key) else {
        return Ok(json!({ "ok": false, "error": "Modelo origen no encontrado." }));
    };
    let (f_fr, val_fr) = reconstruct_fr_curve(src)?;
    let curve = downsample_curve(&f_fr, &val_fr, 64);
    Ok(json!({
        "ok": true,
        "entry": entry_to_json(src),
        "name": src.name,
        "stock_fr_curve": curve,
        "source_fr": curve,
    }))
}

fn cmd_autoeq_match(
    src_key: &str,
    tgt_key: &str,
    max_freq: f64,
    max_bands: usize,
    max_gain: f64,
    supports_shelves: bool,
) -> Result<Value, String> {
    let (entries, _, _) = load_catalog(false);
    let Some(src) = find_entry_by_key(&entries, src_key) else {
        return Ok(json!({
            "ok": false,
            "error": "Modelo origen no encontrado en el catálogo."
        }));
    };
    let Some(tgt) = find_entry_by_key(&entries, tgt_key) else {
        return Ok(json!({
            "ok": false,
            "error": "Modelo destino no encontrado en el catálogo."
        }));
    };

    let m = compute_match_headphone_peq(src, tgt, max_freq, max_bands, max_gain, supports_shelves)?;
    let rmse_round = ((m.rmse * 100.0).round()) / 100.0;
    let stock_ds = downsample_curve(&m.freqs, &m.stock_fr, 64);
    let target_fr_ds = downsample_curve(&m.freqs, &m.target_fr_aligned, 64);
    let delta_ds = downsample_curve(&m.freqs, &m.delta_db, 64);

    Ok(json!({
        "ok": true,
        "bands": m.bands,
        "rmse": rmse_round,
        "warning": m.warning,
        "target_curve": delta_ds,
        "stock_fr_curve": stock_ds,
        "target_fr_curve": target_fr_ds,
        "source_fr": stock_ds,
        "target_fr": target_fr_ds,
        "target_label": tgt.name,
        "message": format!("{} → {} (RMSE {:.2} dB)", src.name, tgt.name, m.rmse),
    }))
}

fn cmd_autoeq_target(
    src_key: &str,
    target_arg: &str,
    max_freq: f64,
    max_bands: usize,
    max_gain: f64,
    supports_shelves: bool,
) -> Result<Value, String> {
    let (entries, _, _) = load_catalog(false);
    let Some(src) = find_entry_by_key(&entries, src_key) else {
        return Ok(json!({
            "ok": false,
            "error": "Modelo origen no encontrado en el catálogo."
        }));
    };

    let csv_opt = resolve_target_csv_arg(target_arg);
    let m = compute_target_curve_peq(
        src,
        csv_opt.as_deref(),
        max_freq,
        max_bands,
        max_gain,
        supports_shelves,
    )?;
    let rmse_round = ((m.rmse * 100.0).round()) / 100.0;
    let stock_ds = downsample_curve(&m.freqs, &m.stock_fr, 64);
    let target_fr_ds = downsample_curve(&m.freqs, &m.target_fr_aligned, 64);
    let delta_ds = downsample_curve(&m.freqs, &m.delta_db, 64);
    let label = csv_opt
        .as_deref()
        .map(|s| s.trim_end_matches(".csv").to_string())
        .unwrap_or_else(|| "AutoEq Target".to_string());

    Ok(json!({
        "ok": true,
        "bands": m.bands,
        "rmse": rmse_round,
        "target_curve": delta_ds,
        "stock_fr_curve": stock_ds,
        "target_fr_curve": target_fr_ds,
        "source_fr": stock_ds,
        "target_fr": target_fr_ds,
        "target_label": label,
        "message": format!("{} → Target (RMSE {:.2} dB)", src.name, m.rmse),
    }))
}

fn cmd_bt_spp(mac: &str, action: &str, preset_idx: u8, bands_json: &str) -> Result<Value, String> {
    match action {
        "preset" => {
            platform_linux::bt_spp_set_preset(mac, preset_idx)?;
            Ok(json!({
                "ok": true,
                "message": format!("Preset Bluetooth SPP #{preset_idx} enviado.")
            }))
        }
        "read" => {
            let bands = platform_linux::bt_spp_read_eq(mac)?;
            Ok(json!({
                "ok": true,
                "bands": bands,
                "message": "Leído EQ desde firmware Bluetooth."
            }))
        }
        "write" => {
            let bands = parse_bands_json(bands_json)?;
            platform_linux::bt_spp_write_eq(mac, &bands)?;
            Ok(json!({
                "ok": true,
                "message": "EQ guardado en firmware Bluetooth SPP."
            }))
        }
        other => Ok(json!({
            "ok": false,
            "error": format!("Acción SPP desconocida: {other}")
        })),
    }
}

// ---------------------------------------------------------------------------
// Minimal Zero-Bloat Flag Parser supporting `--flag val` and `--flag=val`
// ---------------------------------------------------------------------------

struct CliArgs {
    cmd: String,
    pos: Vec<String>,
    kv: HashMap<String, String>,
    bools: HashSet<String>,
}

const BOOL_FLAGS: &[&str] = &[
    "--refresh-db",
    "--apply",
    "--commit",
    "--bypass",
    "--moondrop-only",
    "--in-ear-only",
    "--no-shelves",
    "--help",
    "-h",
    "--version",
    "-V",
];

impl CliArgs {
    fn parse(raw_args: &[String]) -> Result<Self, String> {
        if raw_args.is_empty() {
            return Err("Subcomando requerido (eq | status | poll | waybar-status | apply | flat | search | source-fr | autoeq-match | autoeq-target | preset-save | preset-delete | import-apo | export-apo | bt-spp)".to_string());
        }
        let cmd = raw_args[0].clone();
        let mut pos = Vec::new();
        let mut kv = HashMap::new();
        let mut bools = HashSet::new();

        let mut i = 1;
        while i < raw_args.len() {
            let arg = &raw_args[i];
            if let Some((k, v)) = arg.split_once('=') {
                if BOOL_FLAGS.contains(&k) {
                    if v != "false" && v != "0" {
                        bools.insert(k.to_string());
                    }
                } else {
                    kv.insert(k.to_string(), v.to_string());
                }
                i += 1;
            } else if BOOL_FLAGS.contains(&arg.as_str()) {
                bools.insert(arg.clone());
                i += 1;
            } else if arg.starts_with("--") {
                if i + 1 < raw_args.len() {
                    kv.insert(arg.clone(), raw_args[i + 1].clone());
                    i += 2;
                } else {
                    kv.insert(arg.clone(), String::new());
                    i += 1;
                }
            } else {
                pos.push(arg.clone());
                i += 1;
            }
        }
        Ok(Self { cmd, pos, kv, bools })
    }

    fn str_or(&self, key: &str, default: &str) -> String {
        self.kv
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    fn str_or_b64(&self, plain_key: &str, b64_key: &str) -> Result<String, String> {
        if let Some(b) = self.kv.get(b64_key) {
            if !b.is_empty() {
                return decode_b64(b);
            }
        }
        Ok(self.str_or(plain_key, ""))
    }

    fn has(&self, flag: &str) -> bool {
        self.bools.contains(flag)
    }

    fn f64_or(&self, key: &str, default: f64) -> f64 {
        self.kv
            .get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default)
    }

    fn usize_or(&self, key: &str, default: usize) -> usize {
        self.kv
            .get(key)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(default)
    }
}

fn dispatch(args: &CliArgs) -> Result<Value, String> {
    match args.cmd.as_str() {
        "status" => Ok(cmd_status(
            &args.str_or("--device-id", ""),
            args.has("--refresh-db"),
        )),
        "waybar-status" => Ok(cmd_waybar_status()),
        "source-fr" => {
            let src = args.str_or_b64("--src", "--src-b64")?;
            cmd_source_fr(&src)
        }
        "apply" => {
            let bj = args.str_or_b64("--bands-json", "--bands-b64")?;
            cmd_apply(
                &args.str_or("--device-id", ""),
                &bj,
                args.has("--commit"),
                !args.has("--bypass"),
            )
        }
        "flat" => cmd_flat(&args.str_or("--device-id", ""), args.has("--commit")),
        "search" => {
            let q = args.str_or_b64("--q", "--q-b64")?;
            Ok(cmd_search(
                &q,
                args.has("--moondrop-only"),
                args.has("--in-ear-only"),
                args.usize_or("--limit", 80),
            ))
        }
        "autoeq-match" => {
            let src = args.str_or_b64("--src", "--src-b64")?;
            let tgt = args.str_or_b64("--tgt", "--tgt-b64")?;
            cmd_autoeq_match(
                &src,
                &tgt,
                args.f64_or("--max-freq", 10_000.0),
                args.usize_or("--bands", DEFAULT_VIRTUAL_BANDS),
                args.f64_or("--max-gain", 12.0),
                !args.has("--no-shelves"),
            )
        }
        "autoeq-target" => {
            let src = args.str_or_b64("--src", "--src-b64")?;
            let tgt = args.str_or_b64("--target", "--target-b64")?;
            cmd_autoeq_target(
                &src,
                &tgt,
                args.f64_or("--max-freq", 10_000.0),
                args.usize_or("--bands", DEFAULT_VIRTUAL_BANDS),
                args.f64_or("--max-gain", 12.0),
                !args.has("--no-shelves"),
            )
        }
        "preset-save" => {
            let nm = args.str_or_b64("--name", "--name-b64")?;
            let bj = args.str_or_b64("--bands-json", "--bands-b64")?;
            let mut presets = load_presets();
            presets.insert(nm.clone(), parse_bands_json(&bj)?);
            save_presets(&presets)?;
            Ok(json!({
                "ok": true,
                "presets": presets,
                "message": format!("Preset '{nm}' guardado."),
            }))
        }
        "preset-delete" => {
            let nm = args.str_or_b64("--name", "--name-b64")?;
            let mut presets = load_presets();
            presets.remove(&nm);
            save_presets(&presets)?;
            Ok(json!({
                "ok": true,
                "presets": presets,
                "message": format!("Preset '{nm}' eliminado."),
            }))
        }
        "import-apo" => {
            let txt = decode_b64(&args.str_or("--text-b64", ""))?;
            let parsed = parse_parametric_eq(&txt);
            if parsed.is_empty() {
                Ok(json!({
                    "ok": false,
                    "error": "No se encontraron líneas 'Filter N: ON PK/LSC/HSC ...' válidas."
                }))
            } else {
                let max_b = args.usize_or("--bands", DEFAULT_VIRTUAL_BANDS);
                let count = parsed.len().min(max_b);
                let fitted = fit_band_count(&parsed, max_b);
                Ok(json!({
                    "ok": true,
                    "bands": fitted,
                    "message": format!("Importadas {count} bandas APO."),
                }))
            }
        }
        "export-apo" => {
            let bj = decode_b64(&args.str_or("--bands-b64", ""))?;
            let bands = parse_bands_json(&bj)?;
            Ok(json!({
                "ok": true,
                "text": export_parametric_eq(&bands),
            }))
        }
        "bt-spp" => {
            let mac = args.str_or("--mac", "");
            let action = args.str_or("--action", "");
            let idx = args.usize_or("--preset-idx", 0) as u8;
            let bj = if args.kv.contains_key("--bands-b64") {
                args.str_or_b64("--bands-json", "--bands-b64")?
            } else {
                args.str_or("--bands-json", "[]")
            };
            cmd_bt_spp(&mac, &action, idx, &bj)
        }
        other => Err(format!("Comando no soportado: {other}")),
    }
}

fn source_credit_url(e: &AutoEqEntry) -> String {
    if e.source_kind == "squiglink" && !e.squig_base_url.is_empty() {
        e.squig_base_url.trim_end_matches("data/").to_string()
    } else {
        "https://github.com/jaakkopasanen/AutoEq (MIT)".to_string()
    }
}

fn run_human_eq(args: &CliArgs) -> Result<(), String> {
    let devices = discover_all_devices();
    let dev_id = args.str_or("--device-id", "");
    let active = pick_device(&devices, &dev_id);

    let src_query = if !args.pos.is_empty() {
        args.pos.join(" ")
    } else {
        let s = args.str_or("--src", "");
        if !s.is_empty() {
            s
        } else {
            active.map(RuntimeDevice::autoeq_hint).unwrap_or("").to_string()
        }
    };

    if src_query.trim().is_empty() {
        return Err(
            "Indica tu modelo de audífono, p. ej.: autoq-cli eq \"Moondrop Chu 2\" --target \"Harman In-Ear 2019\" [--apply] [--commit] [--out eq.txt]"
                .to_string(),
        );
    }

    let (entries, _, _) = load_catalog(args.has("--refresh-db"));
    let src = find_entry_by_key(&entries, &src_query)
        .or_else(|| find_best_entry_any_brand(&entries, &src_query))
        .ok_or_else(|| format!("No se encontró ningún modelo para '{src_query}'."))?;

    let default_bands = active
        .map(RuntimeDevice::max_bands)
        .unwrap_or(DEFAULT_VIRTUAL_BANDS);
    let max_bands = args.usize_or("--bands", default_bands);
    let max_freq = args.f64_or("--max-freq", 10_000.0);
    let max_gain = args.f64_or("--max-gain", 12.0);
    let supports_shelves = !args.has("--no-shelves");

    let match_query = args.str_or("--match", "");
    let (m, target_desc, extra_credit) = if !match_query.is_empty() {
        let tgt = find_entry_by_key(&entries, &match_query)
            .or_else(|| find_best_entry_any_brand(&entries, &match_query))
            .ok_or_else(|| format!("No se encontró el audífono objetivo '{match_query}'."))?;
        let res = compute_match_headphone_peq(
            src,
            tgt,
            max_freq,
            max_bands,
            max_gain,
            supports_shelves,
        )?;
        (
            res,
            format!("{} [{}]", tgt.name, tgt.source_rig),
            Some(format!("{} — {}", tgt.source_rig, source_credit_url(tgt))),
        )
    } else {
        let raw_target = args.str_or("--target", "Harman In-Ear 2019");
        let csv_opt = resolve_target_csv_arg(&raw_target);
        let res = compute_target_curve_peq(
            src,
            csv_opt.as_deref(),
            max_freq,
            max_bands,
            max_gain,
            supports_shelves,
        )?;
        let label = if raw_target.is_empty() {
            "AutoEq In-Ear (Predeterminado)".to_string()
        } else {
            raw_target
        };
        (res, label, None)
    };

    println!("================================================================");
    println!("  AutoQ CLI (Rust) — Calibración Paramétrica Bajo Demanda");
    println!("================================================================");
    println!("  Origen   : {} [{}]", src.name, src.form_factor());
    println!(
        "  Cortesía : {} ({})",
        src.source_rig,
        source_credit_url(src)
    );
    println!("  Objetivo : {target_desc}");
    if let Some(ec) = &extra_credit {
        println!("  Crédito  : {ec}");
    }
    println!(
        "  Ajuste   : {} bandas | RMSE {:.2} dB | ≤ {:.0} Hz",
        m.bands.len(),
        m.rmse,
        max_freq
    );
    if let Some(w) = &m.warning {
        println!("  {w}");
    }
    println!("----------------------------------------------------------------");
    for (idx, b) in m.bands.iter().enumerate() {
        println!(
            "  Banda {:>2} : {:<4}  {:>6.0} Hz   {:>+5.1} dB   Q {:>5.3}",
            idx + 1,
            b.filter_type.apo_code(),
            b.frequency,
            b.gain,
            b.q
        );
    }
    println!("----------------------------------------------------------------");

    let out_path = args.str_or("--out", "");
    if !out_path.is_empty() {
        let mut txt = format!(
            "# Generado por AutoQ CLI (Rust)\n# Origen: {} — Medición cortesía de {} ({})\n# Objetivo: {} (RMSE {:.2} dB)\n",
            src.name,
            src.source_rig,
            source_credit_url(src),
            target_desc,
            m.rmse
        );
        txt.push_str(&export_parametric_eq(&m.bands));
        std::fs::write(&out_path, txt)
            .map_err(|e| format!("Error guardando '{out_path}': {e}"))?;
        println!("  [OK] Exportado ParametricEQ en: {out_path}");
    }

    let do_commit = args.has("--commit");
    let do_apply = args.has("--apply") || do_commit;
    if do_apply {
        let raw_bands = serde_json::to_string(&m.bands).map_err(|e| e.to_string())?;
        let res = cmd_apply(&dev_id, &raw_bands, do_commit, true)?;
        if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            println!(
                "  [OK] {}",
                res.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("EQ aplicado.")
            );
        } else {
            return Err(res
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("No se pudo aplicar el EQ.")
                .to_string());
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        "autoq-cli {} — Motor unificado en Rust para AutoQ\n\
         Uso:\n\
           autoq-cli eq \"<audífono>\" [--target \"Harman In-Ear 2019\" | --match \"<otro>\"] [--apply] [--commit] [--out eq.txt]\n\
           autoq-cli <subcomando-json> [opciones]\n\n\
         Comandos interactivos de terminal:\n\
           eq               Descarga curva bajo demanda (con atribución), optimiza PEQ y exporta o aplica al hardware\n\
           hid-scan         Lista todos los dispositivos HID conectados (VID:PID y protocolo DSP reconocido)\n\n\
         Subcomandos JSON / Telemetría (UI, Waybar, KDE Plasma):\n\
           poll             Huella instantánea (<1ms) de presencia USB HID / Bluetooth\n\
           waybar-status    Estado JSON compacto (<5ms) para módulo Waybar\n\
           status           Estado completo de dispositivos, bandas y catálogo AutoEQ\n\
           apply            Aplica bandas EQ (USB HID LIVE/COMMIT, PipeWire o Equalizer APO)\n\
           flat             Restablece todas las bandas a 0 dB\n\
           search           Busca entre +14,500 curvas AutoEq + Squiglink\n\
           source-fr        Reconstruye la respuesta en frecuencia stock de un modelo\n\
           autoeq-match     Calcula PEQ para imitar otro audífono del catálogo\n\
           autoeq-target    Calcula PEQ hacia una curva objetivo curada\n\
           preset-save      Guarda un preset de usuario\n\
           preset-delete    Elimina un preset de usuario\n\
           import-apo       Importa texto ParametricEQ.txt (Equalizer APO)\n\
           export-apo       Exporta bandas actuales a ParametricEQ.txt\n\
           bt-spp           Control opcional Bluetooth Classic SPP (preset/read/write)",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() || matches!(raw[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return;
    }
    if matches!(raw[0].as_str(), "-V" | "--version" | "version") {
        println!("autoq-cli {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if raw[0] == "poll" {
        println!("{}", hid::poll_fingerprint());
        return;
    }
    if raw[0] == "hid-scan" {
        let all = hid::scan_all_hid_devices();
        let show_all = raw.iter().any(|a| a == "--all");
        let as_json = raw.iter().any(|a| a == "--json");
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|d| show_all || d.recognized)
            .collect();
        if as_json {
            println!(
                "{}",
                serde_json::to_string(&json!({ "ok": true, "devices": filtered })).unwrap_or_default()
            );
        } else if filtered.is_empty() {
            println!("No se detectaron dispositivos DSP HID compatibles. Usa `autoq-cli hid-scan --all` para ver todos los nodos HID.");
        } else {
            println!("{:<16} {:<10} {:<12} {:<26} NOMBRE HID", "RUTA", "VID:PID", "PROTOCOLO", "MODELO DETECTADO");
            println!("{}", "-".repeat(86));
            for d in &filtered {
                let proto = d
                    .protocol
                    .map(|p| format!("{p:?}"))
                    .unwrap_or_else(|| "desconocido".to_string());
                let matched = d.matched_name.as_deref().unwrap_or("-");
                println!("{:<16} {:<10} {:<12} {:<26} {}", d.path, d.vid_pid, proto, matched, d.hid_name);
            }
        }
        return;
    }
    if raw[0] == "eq" {
        if raw.iter().skip(1).any(|a| a == "-h" || a == "--help") {
            print_help();
            return;
        }
        if let Err(e) = CliArgs::parse(&raw).and_then(|a| run_human_eq(&a)) {
            eprintln!("[ERROR] {e}");
            std::process::exit(1);
        }
        return;
    }

    let out = match CliArgs::parse(&raw).and_then(|a| dispatch(&a)) {
        Ok(v) => v,
        Err(e) => json!({ "ok": false, "error": e }),
    };

    if let Ok(line) = serde_json::to_string(&out) {
        println!("{line}");
    } else {
        println!(r#"{{"ok":false,"error":"Error serializando salida JSON"}}"#);
    }
}
