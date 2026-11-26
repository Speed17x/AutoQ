//! AutoQ — 100% Native Rust Nothing OS Desktop App & Compact Floating Widget.
//!
//! Calls `autoq-core` directly in-process (zero Python, zero PySide6, zero subprocesses).
//! Supports:
//!   --full                     Full 3-tab Nothing OS app (default)
//!   --widget                   Compact floating Nothing OS widget
//!   --toggle                   Toggle/focus running instance via Unix socket IPC
//!   --install-hyprland-widget  Install Hyprland widget + Waybar config
//!   --install-kde-plasmoid     Install KDE Plasma 6 Plasmoid (org.hector.moondropeq)
//!   --status-waybar            Output single-line JSON for Waybar

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod graph;
mod ipc;
mod theme;

use autoq_core::autoeq::{
    compute_match_headphone_peq, compute_target_curve_peq, downsample_curve, export_parametric_eq,
    find_best_entry_any_brand, load_catalog, normalize_search_text, parse_parametric_eq,
    reconstruct_fr_curve, AutoEqEntry, CURATED_TARGETS, SQUIG_SOURCES,
};
use autoq_core::biquad::{fit_band_count, flat_bands, FilterBand, FilterType, MAX_FREQ_HZ, MAX_Q, MIN_FREQ_HZ, MIN_Q};
use autoq_core::config::{load_presets, save_presets, DEFAULT_VIRTUAL_BANDS};
use autoq_core::hid::{self, MoondropDeviceInfo};
use autoq_core::platform_linux::{self, PipeWireAudioDevice, PW_SINK_NAME};
use autoq_core::platform_windows;
use eframe::egui::{self, CornerRadius, Stroke, Vec2};
use graph::{draw_peq_graph, GraphEvent, GraphState};
use std::collections::BTreeMap;
use std::env;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use theme::{
    apply_nothing_theme, draw_dot_matrix, pill_button, red_cta_button, section_label,
    sub_card_frame, BORDER, MUTED_GRAY, NOTHING_RED, OBSIDIAN, PURE_WHITE, SURFACE,
};

const BRAND_CHIPS: &[(&str, &str)] = &[
    ("Todos", ""),
    ("Moondrop", "moondrop"),
    ("7Hz", "7hz"),
    ("Tanchjim", "tanchjim"),
    ("FiiO", "fiio"),
    ("Sony", "sony"),
    ("Truthear", "truthear"),
    ("KZ", "kz"),
    ("Sennheiser", "sennheiser"),
];

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

    fn badge(&self) -> String {
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

    fn is_usb_hid(&self) -> bool {
        matches!(self, Self::UsbHid(_))
    }

    fn max_bands(&self) -> usize {
        match self {
            Self::UsbHid(d) => d.max_bands,
            Self::PipeWire(p) => p.max_bands,
            Self::WinApo { .. } => DEFAULT_VIRTUAL_BANDS,
        }
    }

    fn max_gain_db(&self) -> f64 {
        match self {
            Self::UsbHid(d) => d.max_gain_db,
            Self::PipeWire(p) => p.max_gain_db,
            Self::WinApo { .. } => 15.0,
        }
    }

    fn supports_shelves(&self) -> bool {
        match self {
            Self::UsbHid(d) => d.supports_shelves,
            Self::PipeWire(p) => p.supports_shelves,
            Self::WinApo { .. } => true,
        }
    }

    fn supports_spp(&self) -> bool {
        matches!(self, Self::PipeWire(p) if p.supports_spp && !p.mac.is_empty())
    }

    fn mac(&self) -> Option<&str> {
        match self {
            Self::PipeWire(p) if !p.mac.is_empty() => Some(&p.mac),
            _ => None,
        }
    }

    fn autoeq_hint(&self) -> &str {
        match self {
            Self::UsbHid(d) => &d.autoeq_hint,
            Self::PipeWire(p) => &p.autoeq_hint,
            Self::WinApo { .. } => "",
        }
    }
}

fn discover_devices() -> Vec<RuntimeDevice> {
    let mut out = Vec::new();
    let usb_objs = hid::scan_moondrop_devices();
    let hid_vps: Vec<String> = usb_objs.iter().map(MoondropDeviceInfo::vid_pid).collect();
    for d in usb_objs {
        out.push(RuntimeDevice::UsbHid(d));
    }
    if cfg!(target_os = "windows") {
        let apo_ready = platform_windows::find_apo_config_dir().is_some();
        out.push(RuntimeDevice::WinApo { apo_ready });
    } else {
        for p in platform_linux::scan_pipewire_audio_sinks(&hid_vps) {
            out.push(RuntimeDevice::PipeWire(p));
        }
    }
    out
}

fn pick_device_idx(devices: &[RuntimeDevice], preferred_id: &str) -> Option<usize> {
    if devices.is_empty() {
        return None;
    }
    if !preferred_id.is_empty() {
        if let Some(i) = devices.iter().position(|d| d.id() == preferred_id) {
            return Some(i);
        }
    }
    if cfg!(target_os = "linux") {
        if let Some(ds) = platform_linux::get_default_sink() {
            if let Some(i) = devices.iter().position(|d| {
                matches!(d, RuntimeDevice::PipeWire(p) if p.sink_name == ds || ds == PW_SINK_NAME)
            }) {
                return Some(i);
            }
        }
    }
    Some(0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Eq,
    AutoQ,
    Advanced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AutoQMode {
    TargetCurve,
    MatchHeadphone,
}

enum BgMsg {
    StatusLoaded {
        devices: Vec<RuntimeDevice>,
        active_idx: Option<usize>,
        bands: Vec<FilterBand>,
        dsp_enabled: bool,
        pipewire_active: bool,
        read_error: Option<String>,
        catalog: Vec<AutoEqEntry>,
        active_sources: usize,
        total_sources: usize,
        auto_src_idx: Option<usize>,
        stock_fr: Vec<[f64; 2]>,
    },
    SourceFrLoaded {
        entry_idx: usize,
        stock_fr: Vec<[f64; 2]>,
    },
    Applied {
        ok: bool,
        committed: bool,
        dsp_enabled: bool,
        pipewire_active: bool,
        bands: Option<Vec<FilterBand>>,
        message: String,
    },
    AutoQComputed {
        ok: bool,
        bands: Vec<FilterBand>,
        rmse: f64,
        warning: Option<String>,
        stock_fr: Vec<[f64; 2]>,
        target_fr: Vec<[f64; 2]>,
        target_delta: Vec<[f64; 2]>,
        target_label: String,
        credit_line: String,
        message: String,
    },
    SppResult {
        ok: bool,
        bands: Option<Vec<FilterBand>>,
        message: String,
    },
    StatusNotice {
        ok: bool,
        message: String,
    },
}

struct AutoQApp {
    // Window mode & IPC
    full_mode: bool,
    active_tab: ActiveTab,
    ipc_rx: Receiver<String>,
    bg_tx: Sender<BgMsg>,
    bg_rx: Receiver<BgMsg>,

    // Audio devices & EQ state
    devices: Vec<RuntimeDevice>,
    active_idx: Option<usize>,
    bands: Vec<FilterBand>,
    selected_band: usize,
    dsp_enabled: bool,
    live_preview: bool,
    overlay_fr: bool,
    pipewire_active: bool,
    dirty_bands: bool,
    busy: bool,
    dragging_node: Option<usize>,
    pending_live_apply: Option<Instant>,

    // Graph curves
    stock_fr: Vec<[f64; 2]>,
    target_fr: Vec<[f64; 2]>,
    target_delta: Vec<[f64; 2]>,
    target_label: String,
    rmse_text: String,

    // AutoEQ catalog & search
    catalog: Vec<AutoEqEntry>,
    active_sources: usize,
    total_sources: usize,
    autoq_mode: AutoQMode,
    src_query: String,
    src_brand_filter: String,
    selected_src_idx: Option<usize>,
    target_curve_idx: usize,
    match_query: String,
    match_brand_filter: String,
    selected_match_idx: Option<usize>,
    max_freq_fit: f64,
    autoq_summary: String,

    // Presets & APO
    presets: BTreeMap<String, Vec<FilterBand>>,
    selected_preset: String,
    new_preset_name: String,
    apo_text: String,

    // Footer telemetry
    status_text: String,
    status_error: bool,
}

impl AutoQApp {
    fn new(cc: &eframe::CreationContext<'_>, start_full: bool, ipc_rx: Receiver<String>) -> Self {
        apply_nothing_theme(&cc.egui_ctx);
        let (bg_tx, bg_rx) = mpsc::channel();
        let presets = load_presets();
        let selected_preset = presets
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "Plano (0 dB)".to_string());

        let mut app = Self {
            full_mode: start_full,
            active_tab: ActiveTab::Eq,
            ipc_rx,
            bg_tx,
            bg_rx,
            devices: Vec::new(),
            active_idx: None,
            bands: flat_bands(DEFAULT_VIRTUAL_BANDS),
            selected_band: 0,
            dsp_enabled: true,
            live_preview: true,
            overlay_fr: true,
            pipewire_active: false,
            dirty_bands: false,
            busy: false,
            dragging_node: None,
            pending_live_apply: None,
            stock_fr: Vec::new(),
            target_fr: Vec::new(),
            target_delta: Vec::new(),
            target_label: String::new(),
            rmse_text: String::new(),
            catalog: Vec::new(),
            active_sources: 0,
            total_sources: 1 + SQUIG_SOURCES.len(),
            autoq_mode: AutoQMode::TargetCurve,
            src_query: String::new(),
            src_brand_filter: String::new(),
            selected_src_idx: None,
            target_curve_idx: 1, // Harman In-Ear 2019
            match_query: String::new(),
            match_brand_filter: String::new(),
            selected_match_idx: None,
            max_freq_fit: 10_000.0,
            autoq_summary: String::new(),
            presets,
            selected_preset,
            new_preset_name: String::new(),
            apo_text: String::new(),
            status_text: "INICIANDO MOTOR RUST...".to_string(),
            status_error: false,
        };
        app.spawn_refresh(cc.egui_ctx.clone(), String::new(), false);
        app
    }

    fn active_device(&self) -> Option<&RuntimeDevice> {
        self.active_idx.and_then(|i| self.devices.get(i))
    }

    fn set_status(&mut self, msg: impl Into<String>, error: bool) {
        self.status_text = msg.into();
        self.status_error = error;
    }

    fn set_window_mode(&mut self, ctx: &egui::Context, full: bool) {
        self.full_mode = full;
        let sz = if full {
            Vec2::new(500.0, 720.0)
        } else {
            Vec2::new(390.0, 365.0)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(sz));
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn spawn_refresh(&mut self, ctx: egui::Context, preferred_id: String, refresh_db: bool) {
        self.busy = true;
        self.set_status(
            if refresh_db {
                "SINCRONIZANDO 8 BASES FR EN PARALELO..."
            } else {
                "ESCANEANDO HARDWARE DSP Y AUDIO..."
            },
            false,
        );
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let devices = discover_devices();
            let active_idx = pick_device_idx(&devices, &preferred_id);
            let pw_active = platform_linux::is_pipewire_eq_running();
            let mut dsp_enabled = true;
            let mut read_error = None;
            let mut bands = flat_bands(
                active_idx
                    .and_then(|i| devices.get(i))
                    .map(RuntimeDevice::max_bands)
                    .unwrap_or(DEFAULT_VIRTUAL_BANDS),
            );

            if let Some(dev) = active_idx.and_then(|i| devices.get(i)) {
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

            let (catalog, active_sources, total_sources) = load_catalog(refresh_db);
            let hint = active_idx
                .and_then(|i| devices.get(i))
                .map(RuntimeDevice::autoeq_hint)
                .unwrap_or("");
            let auto_src_idx = if !hint.is_empty() {
                find_best_entry_any_brand(&catalog, hint).and_then(|best| {
                    let bk = best.unique_key();
                    catalog.iter().position(|e| e.unique_key() == bk)
                })
            } else {
                None
            };
            let stock_fr = auto_src_idx
                .and_then(|i| catalog.get(i))
                .and_then(|e| reconstruct_fr_curve(e).ok())
                .map(|(f, v)| downsample_curve(&f, &v, 64))
                .unwrap_or_default();

            let _ = tx.send(BgMsg::StatusLoaded {
                devices,
                active_idx,
                bands,
                dsp_enabled,
                pipewire_active: pw_active,
                read_error,
                catalog,
                active_sources,
                total_sources,
                auto_src_idx,
                stock_fr,
            });
            ctx.request_repaint();
        });
    }

    fn spawn_apply(&mut self, ctx: egui::Context, commit: bool) {
        let Some(dev) = self.active_device().cloned() else {
            self.set_status("SIN DISPOSITIVO DE AUDIO CONECTADO", true);
            return;
        };
        let fitted = fit_band_count(&self.bands, dev.max_bands());
        let enable_eq = self.dsp_enabled;
        if commit {
            self.busy = true;
            self.set_status("GUARDANDO EN MEMORIA FLASH DEL DSP...", false);
        }
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let msg = match dev {
                RuntimeDevice::UsbHid(info) => {
                    match hid::write_device_eq(&fitted, commit, enable_eq, Some(&info)) {
                        Ok(_) => BgMsg::Applied {
                            ok: true,
                            committed: commit,
                            dsp_enabled: enable_eq,
                            pipewire_active: false,
                            bands: Some(fitted),
                            message: if commit {
                                format!("GUARDADO EN FLASH ({})", info.display_name)
                            } else if enable_eq {
                                format!("DSP LIVE ACTIVO ({})", info.display_name)
                            } else {
                                format!("DSP EN BYPASS ({})", info.display_name)
                            },
                        },
                        Err(e) => BgMsg::Applied {
                            ok: false,
                            committed: false,
                            dsp_enabled: enable_eq,
                            pipewire_active: false,
                            bands: None,
                            message: e,
                        },
                    }
                }
                RuntimeDevice::PipeWire(pw) => {
                    match platform_linux::apply_pipewire_eq(&pw, &fitted, enable_eq) {
                        Ok(active) => BgMsg::Applied {
                            ok: true,
                            committed: true,
                            dsp_enabled: active,
                            pipewire_active: active,
                            bands: Some(fitted),
                            message: if active {
                                format!("PIPEWIRE 10B ACTIVO ({})", pw.display_name)
                            } else {
                                format!("EQ EN BYPASS ({})", pw.display_name)
                            },
                        },
                        Err(e) => BgMsg::Applied {
                            ok: false,
                            committed: false,
                            dsp_enabled: enable_eq,
                            pipewire_active: false,
                            bands: None,
                            message: e,
                        },
                    }
                }
                RuntimeDevice::WinApo { .. } => {
                    match platform_windows::apply_apo_eq(&fitted, enable_eq) {
                        Ok(_) => BgMsg::Applied {
                            ok: true,
                            committed: true,
                            dsp_enabled: enable_eq,
                            pipewire_active: false,
                            bands: Some(fitted),
                            message: if enable_eq {
                                "EQUALIZER APO 10B APLICADO".to_string()
                            } else {
                                "EQUALIZER APO EN BYPASS".to_string()
                            },
                        },
                        Err(e) => BgMsg::Applied {
                            ok: false,
                            committed: false,
                            dsp_enabled: enable_eq,
                            pipewire_active: false,
                            bands: None,
                            message: e,
                        },
                    }
                }
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn spawn_load_source_fr(&mut self, ctx: egui::Context, entry_idx: usize) {
        let Some(entry) = self.catalog.get(entry_idx).cloned() else {
            return;
        };
        self.selected_src_idx = Some(entry_idx);
        self.set_status(format!("CARGANDO CURVA FR: {}...", entry.name), false);
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            if let Ok((f, v)) = reconstruct_fr_curve(&entry) {
                let _ = tx.send(BgMsg::SourceFrLoaded {
                    entry_idx,
                    stock_fr: downsample_curve(&f, &v, 64),
                });
                ctx.request_repaint();
            }
        });
    }

    fn spawn_autoq(&mut self, ctx: egui::Context) {
        let Some(src_entry) = self.selected_src_idx.and_then(|i| self.catalog.get(i)).cloned() else {
            self.set_status("SELECCIONA PRIMERO TU AUDÍFONO ORIGEN", true);
            return;
        };
        let max_bands = self
            .active_device()
            .map(RuntimeDevice::max_bands)
            .unwrap_or(DEFAULT_VIRTUAL_BANDS);
        let max_gain = self
            .active_device()
            .map(RuntimeDevice::max_gain_db)
            .unwrap_or(12.0);
        let shelves = self
            .active_device()
            .map(RuntimeDevice::supports_shelves)
            .unwrap_or(true);
        let max_freq = self.max_freq_fit;
        let mode = self.autoq_mode;
        let target_idx = self.target_curve_idx;
        let match_entry = self.selected_match_idx.and_then(|i| self.catalog.get(i)).cloned();

        self.busy = true;
        self.set_status(format!("OPTIMIZANDO {max_bands} BANDAS EN RUST..."), false);
        let tx = self.bg_tx.clone();

        thread::spawn(move || {
            let msg = match mode {
                AutoQMode::TargetCurve => {
                    let (t_label, t_file) = CURATED_TARGETS
                        .get(target_idx)
                        .copied()
                        .unwrap_or(("Harman In-Ear 2019", "Harman in-ear 2019.csv"));
                    let csv_opt = if t_file.is_empty() { None } else { Some(t_file) };
                    match compute_target_curve_peq(&src_entry, csv_opt, max_freq, max_bands, max_gain, shelves) {
                        Ok(m) => BgMsg::AutoQComputed {
                            ok: true,
                            bands: m.bands,
                            rmse: m.rmse,
                            warning: m.warning,
                            stock_fr: downsample_curve(&m.freqs, &m.stock_fr, 64),
                            target_fr: downsample_curve(&m.freqs, &m.target_fr_aligned, 64),
                            target_delta: downsample_curve(&m.freqs, &m.delta_db, 64),
                            target_label: t_label.to_string(),
                            credit_line: format!("Medición cortesía de: {}", src_entry.source_rig),
                            message: format!("{} → {} (RMSE {:.2} dB)", src_entry.name, t_label, m.rmse),
                        },
                        Err(e) => BgMsg::AutoQComputed {
                            ok: false,
                            bands: Vec::new(),
                            rmse: 0.0,
                            warning: None,
                            stock_fr: Vec::new(),
                            target_fr: Vec::new(),
                            target_delta: Vec::new(),
                            target_label: String::new(),
                            credit_line: String::new(),
                            message: e,
                        },
                    }
                }
                AutoQMode::MatchHeadphone => {
                    let Some(tgt_entry) = match_entry else {
                        let _ = tx.send(BgMsg::StatusNotice {
                            ok: false,
                            message: "SELECCIONA EL AUDÍFONO DESTINO A IMITAR".to_string(),
                        });
                        ctx.request_repaint();
                        return;
                    };
                    match compute_match_headphone_peq(&src_entry, &tgt_entry, max_freq, max_bands, max_gain, shelves) {
                        Ok(m) => BgMsg::AutoQComputed {
                            ok: true,
                            bands: m.bands,
                            rmse: m.rmse,
                            warning: m.warning,
                            stock_fr: downsample_curve(&m.freqs, &m.stock_fr, 64),
                            target_fr: downsample_curve(&m.freqs, &m.target_fr_aligned, 64),
                            target_delta: downsample_curve(&m.freqs, &m.delta_db, 64),
                            target_label: tgt_entry.name.clone(),
                            credit_line: format!(
                                "Mediciones: {} / {}",
                                src_entry.source_rig, tgt_entry.source_rig
                            ),
                            message: format!(
                                "{} → {} (RMSE {:.2} dB)",
                                src_entry.name, tgt_entry.name, m.rmse
                            ),
                        },
                        Err(e) => BgMsg::AutoQComputed {
                            ok: false,
                            bands: Vec::new(),
                            rmse: 0.0,
                            warning: None,
                            stock_fr: Vec::new(),
                            target_fr: Vec::new(),
                            target_delta: Vec::new(),
                            target_label: String::new(),
                            credit_line: String::new(),
                            message: e,
                        },
                    }
                }
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn apply_preset_by_name(&mut self, ctx: &egui::Context, name: &str) {
        let Some(raw) = self.presets.get(name).cloned() else {
            return;
        };
        let n = self
            .active_device()
            .map(RuntimeDevice::max_bands)
            .unwrap_or(DEFAULT_VIRTUAL_BANDS);
        self.selected_preset = name.to_string();
        self.bands = fit_band_count(&raw, n);
        self.selected_band = 0;
        self.dsp_enabled = true;
        self.target_delta.clear();
        self.target_fr.clear();
        self.target_label.clear();
        self.rmse_text.clear();
        self.dirty_bands = true;
        if self.active_device().is_some() {
            self.spawn_apply(ctx.clone(), false);
        }
    }

    fn cycle_preset(&mut self, ctx: &egui::Context, delta: isize) {
        let keys: Vec<String> = self.presets.keys().cloned().collect();
        if keys.is_empty() {
            return;
        }
        let cur = keys
            .iter()
            .position(|k| k == &self.selected_preset)
            .unwrap_or(0) as isize;
        let next = (cur + delta).rem_euclid(keys.len() as isize) as usize;
        let name = keys[next].clone();
        self.apply_preset_by_name(ctx, &name);
    }

    fn filter_catalog_indices(&self, query: &str, brand: &str, limit: usize) -> Vec<usize> {
        let combined = format!("{brand} {query}");
        let norm_q = normalize_search_text(&combined);
        let tokens: Vec<&str> = norm_q.split_whitespace().collect();
        let mut out = Vec::with_capacity(limit);
        for (idx, e) in self.catalog.iter().enumerate() {
            if tokens.is_empty() {
                out.push(idx);
            } else {
                let hay = normalize_search_text(&format!("{} {}", e.name, e.source_rig));
                if tokens.iter().all(|t| hay.contains(t)) {
                    out.push(idx);
                }
            }
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    fn poll_channels(&mut self, ctx: &egui::Context) {
        while let Ok(cmd) = self.ipc_rx.try_recv() {
            match cmd.as_str() {
                "full" => self.set_window_mode(ctx, true),
                "widget" => self.set_window_mode(ctx, false),
                "toggle" => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                _ => {}
            }
        }

        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMsg::StatusLoaded {
                    devices,
                    active_idx,
                    bands,
                    dsp_enabled,
                    pipewire_active,
                    read_error,
                    catalog,
                    active_sources,
                    total_sources,
                    auto_src_idx,
                    stock_fr,
                } => {
                    self.busy = false;
                    self.devices = devices;
                    self.active_idx = active_idx;
                    self.bands = bands;
                    self.selected_band = self.selected_band.min(self.bands.len().saturating_sub(1));
                    self.dsp_enabled = dsp_enabled;
                    self.pipewire_active = pipewire_active;
                    self.catalog = catalog;
                    self.active_sources = active_sources;
                    self.total_sources = total_sources;
                    if self.selected_src_idx.is_none() {
                        self.selected_src_idx = auto_src_idx;
                    }
                    if !stock_fr.is_empty() {
                        self.stock_fr = stock_fr;
                    }
                    if let Some(err) = read_error {
                        self.set_status(err, true);
                    } else if let Some(dev) = self.active_device() {
                        self.set_status(
                            format!(
                                "{} [{}] · {} CURVAS ({}/{})",
                                dev.display_name().to_uppercase(),
                                dev.badge(),
                                self.catalog.len(),
                                self.active_sources,
                                self.total_sources
                            ),
                            false,
                        );
                    } else {
                        self.set_status("SIN DISPOSITIVO DE AUDIO DETECTADO", true);
                    }
                }
                BgMsg::SourceFrLoaded { entry_idx, stock_fr } => {
                    self.selected_src_idx = Some(entry_idx);
                    self.stock_fr = stock_fr;
                    if let Some(e) = self.catalog.get(entry_idx) {
                        self.set_status(format!("ORIGEN: {}", e.name.to_uppercase()), false);
                    }
                }
                BgMsg::Applied {
                    ok,
                    committed,
                    dsp_enabled,
                    pipewire_active,
                    bands,
                    message,
                } => {
                    self.busy = false;
                    self.dsp_enabled = dsp_enabled;
                    self.pipewire_active = pipewire_active;
                    if let Some(b) = bands {
                        self.bands = b;
                    }
                    if committed {
                        self.dirty_bands = false;
                    }
                    self.set_status(message, !ok);
                }
                BgMsg::AutoQComputed {
                    ok,
                    bands,
                    rmse,
                    warning,
                    stock_fr,
                    target_fr,
                    target_delta,
                    target_label,
                    credit_line,
                    message,
                } => {
                    self.busy = false;
                    if ok {
                        self.bands = bands;
                        self.selected_band = 0;
                        self.dsp_enabled = true;
                        self.dirty_bands = true;
                        self.stock_fr = stock_fr;
                        self.target_fr = target_fr;
                        self.target_delta = target_delta;
                        self.target_label = target_label;
                        self.rmse_text = format!("RMSE {rmse:.2} dB");
                        let mut sum = format!("{message}\n{credit_line}");
                        if let Some(w) = warning {
                            sum.push_str(&format!("\n⚠ {w}"));
                        }
                        self.autoq_summary = sum;
                        self.set_status(message.to_uppercase(), false);
                        if self.live_preview && self.active_device().is_some() {
                            self.spawn_apply(ctx.clone(), false);
                        }
                    } else {
                        self.autoq_summary = message.clone();
                        self.set_status(message, true);
                    }
                }
                BgMsg::SppResult { ok, bands, message } => {
                    self.busy = false;
                    if let Some(b) = bands {
                        let n = self
                            .active_device()
                            .map(RuntimeDevice::max_bands)
                            .unwrap_or(DEFAULT_VIRTUAL_BANDS);
                        self.bands = fit_band_count(&b, n);
                    }
                    self.set_status(message, !ok);
                }
                BgMsg::StatusNotice { ok, message } => {
                    self.busy = false;
                    self.set_status(message, !ok);
                }
            }
        }

        // Debounced live preview apply after dragging sliders/nodes
        if let Some(when) = self.pending_live_apply {
            if Instant::now() >= when {
                self.pending_live_apply = None;
                if self.live_preview && self.dsp_enabled && self.active_device().is_some() {
                    self.spawn_apply(ctx.clone(), false);
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(60));
            }
        }
    }

    fn schedule_live_apply(&mut self) {
        self.dirty_bands = true;
        if self.live_preview && self.dsp_enabled {
            self.pending_live_apply = Some(Instant::now() + Duration::from_millis(110));
        }
    }

    fn render_compact_widget(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            draw_dot_matrix(ui, "AUTOQ", 2.6, PURE_WHITE);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill_button(ui, "⤢", false).on_hover_text("Abrir app completa").clicked() {
                    self.set_window_mode(ctx, true);
                }
                let on_lbl = if self.dsp_enabled { "● ON" } else { "○ OFF" };
                if pill_button(ui, on_lbl, self.dsp_enabled).clicked() {
                    self.dsp_enabled = !self.dsp_enabled;
                    self.spawn_apply(ctx.clone(), false);
                }
                if let Some(dev) = self.active_device() {
                    ui.label(
                        egui::RichText::new(dev.badge())
                            .size(10.0)
                            .monospace()
                            .strong()
                            .color(MUTED_GRAY),
                    );
                }
            });
        });

        ui.add_space(4.0);
        let max_g = self
            .active_device()
            .map(RuntimeDevice::max_gain_db)
            .unwrap_or(12.0);
        let ev = draw_peq_graph(
            ui,
            &GraphState {
                bands: &self.bands,
                selected_band: self.selected_band,
                dsp_enabled: self.dsp_enabled,
                overlay_fr: self.overlay_fr,
                stock_fr: &self.stock_fr,
                target_fr: &self.target_fr,
                target_delta: &self.target_delta,
                target_label: &self.target_label,
                rmse_text: &self.rmse_text,
                max_gain_db: max_g,
                height: 175.0,
            },
            &mut self.dragging_node,
        );
        self.handle_graph_event(ev);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if pill_button(ui, "<", false).clicked() {
                self.cycle_preset(ctx, -1);
            }
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(&self.selected_preset)
                        .size(11.5)
                        .strong()
                        .color(PURE_WHITE),
                );
            });
            if pill_button(ui, ">", false).clicked() {
                self.cycle_preset(ctx, 1);
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if pill_button(ui, "0 dB", false).clicked() {
                let n = self
                    .active_device()
                    .map(RuntimeDevice::max_bands)
                    .unwrap_or(DEFAULT_VIRTUAL_BANDS);
                self.bands = flat_bands(n);
                self.dsp_enabled = true;
                self.spawn_apply(ctx.clone(), false);
            }
            if pill_button(ui, "SYNC", false).clicked() {
                let id = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
                self.spawn_refresh(ctx.clone(), id, false);
            }
            if self.active_device().map(RuntimeDevice::is_usb_hid).unwrap_or(false) {
                if red_cta_button(ui, "GUARDAR FLASH").clicked() {
                    self.spawn_apply(ctx.clone(), true);
                }
            }
        });
    }

    fn handle_graph_event(&mut self, ev: GraphEvent) {
        match ev {
            GraphEvent::None => {}
            GraphEvent::Select(idx) => {
                if idx < self.bands.len() {
                    self.selected_band = idx;
                }
            }
            GraphEvent::DragBand {
                idx,
                freq,
                gain,
                released,
            } => {
                if let Some(b) = self.bands.get_mut(idx) {
                    self.selected_band = idx;
                    b.frequency = freq;
                    b.gain = gain;
                    self.schedule_live_apply();
                    if released {
                        self.pending_live_apply = Some(Instant::now());
                    }
                }
            }
            GraphEvent::ScrollQ { idx, q } => {
                if let Some(b) = self.bands.get_mut(idx) {
                    self.selected_band = idx;
                    b.q = q;
                    self.schedule_live_apply();
                }
            }
        }
    }

    fn render_full_app(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // ── Top Nothing OS Dot-Matrix Header ──
        ui.horizontal(|ui| {
            draw_dot_matrix(ui, "AUTOQ", 2.8, PURE_WHITE);
            ui.label(
                egui::RichText::new("RUST")
                    .size(9.5)
                    .strong()
                    .monospace()
                    .color(NOTHING_RED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill_button(ui, "⤡", false).on_hover_text("Modo widget compacto").clicked() {
                    self.set_window_mode(ctx, false);
                }
                let on_lbl = if self.dsp_enabled { "● EQ ON" } else { "○ BYPASS" };
                if pill_button(ui, on_lbl, self.dsp_enabled).clicked() {
                    self.dsp_enabled = !self.dsp_enabled;
                    self.spawn_apply(ctx.clone(), false);
                }
                if pill_button(ui, "↻", false).on_hover_text("Re-escanear hardware").clicked() {
                    let id = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
                    self.spawn_refresh(ctx.clone(), id, false);
                }
            });
        });

        ui.add_space(4.0);

        // ── Device Selector Row ──
        let cur_label = self
            .active_device()
            .map(|d| format!("[{}] {}", d.badge(), d.display_name()))
            .unwrap_or_else(|| "Sin dispositivo de audio".to_string());
        let mut chosen_idx = self.active_idx;
        egui::ComboBox::from_id_salt("device_selector")
            .width(ui.available_width())
            .selected_text(cur_label)
            .show_ui(ui, |ui| {
                for (i, d) in self.devices.iter().enumerate() {
                    let lbl = format!("[{}] {}", d.badge(), d.display_name());
                    if ui.selectable_label(self.active_idx == Some(i), lbl).clicked() {
                        chosen_idx = Some(i);
                    }
                }
            });
        if chosen_idx != self.active_idx {
            self.active_idx = chosen_idx;
            let pref = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
            self.spawn_refresh(ctx.clone(), pref, false);
        }

        ui.add_space(6.0);

        // ── 3-Tab Nothing OS Pill Bar ──
        egui::Frame::NONE
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                ui.columns(3, |cols| {
                    if cols[0]
                        .centered_and_justified(|ui| pill_button(ui, "EQ", self.active_tab == ActiveTab::Eq))
                        .inner
                        .clicked()
                    {
                        self.active_tab = ActiveTab::Eq;
                    }
                    if cols[1]
                        .centered_and_justified(|ui| pill_button(ui, "AUTOQ", self.active_tab == ActiveTab::AutoQ))
                        .inner
                        .clicked()
                    {
                        self.active_tab = ActiveTab::AutoQ;
                    }
                    if cols[2]
                        .centered_and_justified(|ui| pill_button(ui, "AVANZADO", self.active_tab == ActiveTab::Advanced))
                        .inner
                        .clicked()
                    {
                        self.active_tab = ActiveTab::Advanced;
                    }
                });
            });

        ui.add_space(6.0);

        // ── Tab Body ──
        egui::ScrollArea::vertical()
            .id_salt("main_tab_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.active_tab {
                ActiveTab::Eq => self.render_eq_tab(ui, ctx),
                ActiveTab::AutoQ => self.render_autoq_tab(ui, ctx),
                ActiveTab::Advanced => self.render_advanced_tab(ui, ctx),
            });
    }

    fn render_eq_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Quick action strip
        ui.horizontal(|ui| {
            let fr_lbl = if self.overlay_fr { "FR" } else { "ΔEQ" };
            if pill_button(ui, fr_lbl, self.overlay_fr).clicked() {
                self.overlay_fr = !self.overlay_fr;
            }
            if pill_button(ui, "LIVE", self.live_preview).clicked() {
                self.live_preview = !self.live_preview;
            }
            if pill_button(ui, "0 dB", false).clicked() {
                let n = self
                    .active_device()
                    .map(RuntimeDevice::max_bands)
                    .unwrap_or(DEFAULT_VIRTUAL_BANDS);
                self.bands = flat_bands(n);
                self.target_delta.clear();
                self.target_fr.clear();
                self.target_label.clear();
                self.rmse_text.clear();
                self.dsp_enabled = true;
                self.spawn_apply(ctx.clone(), false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_usb = self.active_device().map(RuntimeDevice::is_usb_hid).unwrap_or(false);
                let cta = if is_usb {
                    if self.dirty_bands { "GUARDAR FLASH ●" } else { "GUARDAR FLASH" }
                } else {
                    "APLICAR EQ"
                };
                if red_cta_button(ui, cta).clicked() {
                    self.spawn_apply(ctx.clone(), is_usb);
                }
            });
        });

        ui.add_space(4.0);

        // Interactive Graph
        let max_g = self
            .active_device()
            .map(RuntimeDevice::max_gain_db)
            .unwrap_or(12.0);
        let ev = draw_peq_graph(
            ui,
            &GraphState {
                bands: &self.bands,
                selected_band: self.selected_band,
                dsp_enabled: self.dsp_enabled,
                overlay_fr: self.overlay_fr,
                stock_fr: &self.stock_fr,
                target_fr: &self.target_fr,
                target_delta: &self.target_delta,
                target_label: &self.target_label,
                rmse_text: &self.rmse_text,
                max_gain_db: max_g,
                height: 220.0,
            },
            &mut self.dragging_node,
        );
        self.handle_graph_event(ev);

        ui.add_space(6.0);

        // Band selector chips
        ui.horizontal_wrapped(|ui| {
            for i in 0..self.bands.len() {
                let b = &self.bands[i];
                let lbl = format!("B{} {:+.1}", i + 1, b.gain);
                if pill_button(ui, &lbl, i == self.selected_band).clicked() {
                    self.selected_band = i;
                }
            }
        });

        ui.add_space(6.0);

        // Selected Band Controls Card
        let supports_shelves = self
            .active_device()
            .map(RuntimeDevice::supports_shelves)
            .unwrap_or(true);
        let mut changed = false;

        sub_card_frame().show(ui, |ui| {
            let idx = self.selected_band.min(self.bands.len().saturating_sub(1));
            if let Some(b) = self.bands.get_mut(idx) {
                ui.horizontal(|ui| {
                    section_label(ui, &format!("BANDA {} — AJUSTE FINO", idx + 1));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if supports_shelves {
                            if pill_button(ui, "HSC", b.filter_type == FilterType::HighShelf).clicked() {
                                b.filter_type = FilterType::HighShelf;
                                changed = true;
                            }
                            if pill_button(ui, "LSC", b.filter_type == FilterType::LowShelf).clicked() {
                                b.filter_type = FilterType::LowShelf;
                                changed = true;
                            }
                        }
                        if pill_button(ui, "PK", b.filter_type == FilterType::Peaking).clicked() {
                            b.filter_type = FilterType::Peaking;
                            changed = true;
                        }
                    });
                });

                ui.add_space(4.0);
                egui::Grid::new("band_sliders_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("FREQ (Hz)").size(10.5).monospace().color(MUTED_GRAY));
                        let mut f = b.frequency;
                        if ui
                            .add(egui::Slider::new(&mut f, MIN_FREQ_HZ..=MAX_FREQ_HZ).logarithmic(true).fixed_decimals(0))
                            .changed()
                        {
                            b.frequency = f.round();
                            changed = true;
                        }
                        ui.end_row();

                        ui.label(egui::RichText::new("GAIN (dB)").size(10.5).monospace().color(MUTED_GRAY));
                        let mut g = b.gain;
                        if ui
                            .add(egui::Slider::new(&mut g, -max_g..=max_g).step_by(0.1).fixed_decimals(1))
                            .changed()
                        {
                            b.gain = (g * 10.0).round() / 10.0;
                            changed = true;
                        }
                        ui.end_row();

                        ui.label(egui::RichText::new("FACTOR Q").size(10.5).monospace().color(MUTED_GRAY));
                        let mut q = b.q;
                        if ui
                            .add(egui::Slider::new(&mut q, MIN_Q..=MAX_Q).logarithmic(true).fixed_decimals(2))
                            .changed()
                        {
                            b.q = (q * 100.0).round() / 100.0;
                            changed = true;
                        }
                        ui.end_row();
                    });
            }
        });

        if changed {
            self.schedule_live_apply();
        }
    }

    fn render_autoq_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Source headphone picker card
        sub_card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                section_label(
                    ui,
                    &format!("1. TU AUDÍFONO ACTUAL ({} CURVAS)", self.catalog.len()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(ui, "SYNC 8 DB", false).clicked() {
                        let id = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
                        self.spawn_refresh(ctx.clone(), id, true);
                    }
                });
            });

            let cur_src_lbl = self
                .selected_src_idx
                .and_then(|i| self.catalog.get(i))
                .map(|e| format!("{} [{}]", e.name, e.source_rig))
                .unwrap_or_else(|| "Ninguno seleccionado".to_string());
            ui.label(egui::RichText::new(cur_src_lbl).size(11.5).strong().color(NOTHING_RED));

            ui.add(
                egui::TextEdit::singleline(&mut self.src_query)
                    .hint_text("Buscar tu audífono (ej. Moondrop Chu 2, Golden Ages 2)...")
                    .desired_width(f32::INFINITY),
            );

            ui.horizontal_wrapped(|ui| {
                for &(lbl, tok) in BRAND_CHIPS {
                    if pill_button(ui, lbl, self.src_brand_filter == tok).clicked() {
                        self.src_brand_filter = tok.to_string();
                    }
                }
            });

            let hits = self.filter_catalog_indices(&self.src_query, &self.src_brand_filter, 35);
            egui::ScrollArea::vertical()
                .id_salt("src_results_scroll")
                .max_height(105.0)
                .show(ui, |ui| {
                    for idx in hits {
                        if let Some(e) = self.catalog.get(idx) {
                            let sel = self.selected_src_idx == Some(idx);
                            let lbl = format!("{} [{}]", e.name, e.source_rig);
                            if ui.selectable_label(sel, lbl).clicked() {
                                self.spawn_load_source_fr(ctx.clone(), idx);
                            }
                        }
                    }
                });
        });

        ui.add_space(6.0);

        // Mode selector + Target / Match card
        sub_card_frame().show(ui, |ui| {
            section_label(ui, "2. OBJETIVO DE CALIBRACIÓN");
            ui.horizontal(|ui| {
                if pill_button(ui, "CURVA OBJETIVO", self.autoq_mode == AutoQMode::TargetCurve).clicked() {
                    self.autoq_mode = AutoQMode::TargetCurve;
                }
                if pill_button(ui, "IMITAR AUDÍFONO", self.autoq_mode == AutoQMode::MatchHeadphone).clicked() {
                    self.autoq_mode = AutoQMode::MatchHeadphone;
                }
            });

            ui.add_space(4.0);
            match self.autoq_mode {
                AutoQMode::TargetCurve => {
                    for (i, &(label, _)) in CURATED_TARGETS.iter().enumerate() {
                        if ui.selectable_label(self.target_curve_idx == i, label).clicked() {
                            self.target_curve_idx = i;
                        }
                    }
                }
                AutoQMode::MatchHeadphone => {
                    let cur_tgt_lbl = self
                        .selected_match_idx
                        .and_then(|i| self.catalog.get(i))
                        .map(|e| format!("{} [{}]", e.name, e.source_rig))
                        .unwrap_or_else(|| "Elige el modelo que deseas imitar".to_string());
                    ui.label(egui::RichText::new(cur_tgt_lbl).size(11.0).color(PURE_WHITE));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.match_query)
                            .hint_text("Buscar audífono a imitar (ej. 7Hz Zero 3, Blessing 3)...")
                            .desired_width(f32::INFINITY),
                    );
                    let hits = self.filter_catalog_indices(&self.match_query, &self.match_brand_filter, 35);
                    egui::ScrollArea::vertical()
                        .id_salt("match_results_scroll")
                        .max_height(105.0)
                        .show(ui, |ui| {
                            for idx in hits {
                                if let Some(e) = self.catalog.get(idx) {
                                    let sel = self.selected_match_idx == Some(idx);
                                    let lbl = format!("{} [{}]", e.name, e.source_rig);
                                    if ui.selectable_label(sel, lbl).clicked() {
                                        self.selected_match_idx = Some(idx);
                                    }
                                }
                            }
                        });
                }
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tope Hz:").size(10.5).monospace().color(MUTED_GRAY));
                for (hz, lbl) in [
                    (6000.0, "≤6k"),
                    (8000.0, "≤8k"),
                    (10000.0, "≤10k"),
                    (14000.0, "≤14k"),
                    (20000.0, "≤20k"),
                ] {
                    if pill_button(ui, lbl, (self.max_freq_fit - hz).abs() < 1.0).clicked() {
                        self.max_freq_fit = hz;
                    }
                }
            });

            ui.add_space(6.0);
            let n_bands = self
                .active_device()
                .map(RuntimeDevice::max_bands)
                .unwrap_or(DEFAULT_VIRTUAL_BANDS);
            let btn_lbl = format!("CALCULAR Y APLICAR AUTOQ ({n_bands}B)");
            if ui.add_sized([ui.available_width(), 34.0], egui::Button::new(egui::RichText::new(btn_lbl).strong().color(PURE_WHITE)).fill(NOTHING_RED).corner_radius(CornerRadius::same(14))).clicked() {
                self.spawn_autoq(ctx.clone());
            }

            if !self.autoq_summary.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&self.autoq_summary).size(10.5).color(MUTED_GRAY));
            }
        });
    }

    fn render_advanced_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // ── Presets Card ──
        sub_card_frame().show(ui, |ui| {
            section_label(ui, "PRESETS UNIVERSALES");
            let keys: Vec<String> = self.presets.keys().cloned().collect();
            egui::ScrollArea::vertical()
                .id_salt("presets_scroll")
                .max_height(88.0)
                .show(ui, |ui| {
                    for k in &keys {
                        if ui.selectable_label(&self.selected_preset == k, k).clicked() {
                            self.selected_preset = k.clone();
                        }
                    }
                });

            ui.horizontal(|ui| {
                let sel = self.selected_preset.clone();
                if red_cta_button(ui, "APLICAR").clicked() {
                    self.apply_preset_by_name(ctx, &sel);
                }
                if pill_button(ui, "ELIMINAR", false).clicked() {
                    if self.presets.remove(&sel).is_some() {
                        let _ = save_presets(&self.presets);
                        self.set_status(format!("PRESET '{sel}' ELIMINADO"), false);
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_preset_name)
                        .hint_text("Nombre del nuevo preset...")
                        .desired_width(ui.available_width() - 90.0),
                );
                if pill_button(ui, "GUARDAR", false).clicked() {
                    let nm = self.new_preset_name.trim().to_string();
                    if !nm.is_empty() {
                        self.presets.insert(nm.clone(), self.bands.clone());
                        let _ = save_presets(&self.presets);
                        self.selected_preset = nm.clone();
                        self.new_preset_name.clear();
                        self.set_status(format!("PRESET '{nm}' GUARDADO"), false);
                    }
                }
            });
        });

        ui.add_space(6.0);

        // ── Equalizer APO Import / Export Card ──
        sub_card_frame().show(ui, |ui| {
            section_label(ui, "EQUALIZER APO (PARAMETRICEQ.TXT)");
            ui.add(
                egui::TextEdit::multiline(&mut self.apo_text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .hint_text("Filter 1: ON PK Fc 100 Hz Gain 2.5 dB Q 1.2 ..."),
            );
            ui.horizontal(|ui| {
                if pill_button(ui, "IMPORTAR APO", false).clicked() {
                    let parsed = parse_parametric_eq(&self.apo_text);
                    if parsed.is_empty() {
                        self.set_status("NO SE ENCONTRARON FILTROS APO VÁLIDOS", true);
                    } else {
                        let n = self
                            .active_device()
                            .map(RuntimeDevice::max_bands)
                            .unwrap_or(DEFAULT_VIRTUAL_BANDS);
                        self.bands = fit_band_count(&parsed, n);
                        self.dsp_enabled = true;
                        self.set_status(format!("IMPORTADAS {} BANDAS APO", parsed.len().min(n)), false);
                        if self.live_preview {
                            self.spawn_apply(ctx.clone(), false);
                        }
                    }
                }
                if pill_button(ui, "EXPORTAR Y COPIAR", false).clicked() {
                    let exported = export_parametric_eq(&self.bands);
                    self.apo_text = exported.clone();
                    ctx.copy_text(exported);
                    self.set_status("PARAMETRICEQ.TXT COPIADO AL PORTAPAPELES", false);
                }
            });
        });

        // ── Bluetooth SPP Firmware Card ──
        if let Some(mac) = self.active_device().filter(|d| d.supports_spp()).and_then(|d| d.mac().map(str::to_string)) {
            ui.add_space(6.0);
            sub_card_frame().show(ui, |ui| {
                section_label(ui, "BLUETOOTH SPP FIRMWARE (MOONDROP)");
                ui.horizontal(|ui| {
                    for (idx, lbl) in [(0u8, "FW: POP"), (1u8, "FW: MON"), (2u8, "FW: BASS")] {
                        if pill_button(ui, lbl, false).clicked() {
                            let m = mac.clone();
                            let tx = self.bg_tx.clone();
                            let c = ctx.clone();
                            thread::spawn(move || {
                                let res = platform_linux::bt_spp_set_preset(&m, idx);
                                let _ = tx.send(BgMsg::SppResult {
                                    ok: res.is_ok(),
                                    bands: None,
                                    message: res
                                        .map(|_| format!("PRESET SPP #{idx} ENVIADO"))
                                        .unwrap_or_else(|e| e),
                                });
                                c.request_repaint();
                            });
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if pill_button(ui, "SPP LEER", false).clicked() {
                        let m = mac.clone();
                        let tx = self.bg_tx.clone();
                        let c = ctx.clone();
                        thread::spawn(move || {
                            let msg = match platform_linux::bt_spp_read_eq(&m) {
                                Ok(b) => BgMsg::SppResult {
                                    ok: true,
                                    bands: Some(b),
                                    message: "EQ LEÍDO DESDE FIRMWARE BT SPP".to_string(),
                                },
                                Err(e) => BgMsg::SppResult {
                                    ok: false,
                                    bands: None,
                                    message: e,
                                },
                            };
                            let _ = tx.send(msg);
                            c.request_repaint();
                        });
                    }
                    if red_cta_button(ui, "SPP ESCRIBIR").clicked() {
                        let m = mac.clone();
                        let b = self.bands.clone();
                        let tx = self.bg_tx.clone();
                        let c = ctx.clone();
                        thread::spawn(move || {
                            let res = platform_linux::bt_spp_write_eq(&m, &b);
                            let _ = tx.send(BgMsg::SppResult {
                                ok: res.is_ok(),
                                bands: None,
                                message: res
                                    .map(|_| "EQ GUARDADO EN FIRMWARE BT SPP".to_string())
                                    .unwrap_or_else(|e| e),
                            });
                            c.request_repaint();
                        });
                    }
                });
            });
        }

        // ── Linux 1-Click Integration Card ──
        if !cfg!(target_os = "windows") {
            ui.add_space(6.0);
            sub_card_frame().show(ui, |ui| {
                section_label(ui, "INTEGRACIÓN LINUX Y PERMISOS USB (1 CLIC)");
                ui.horizontal(|ui| {
                    if pill_button(ui, "WIDGET HYPRLAND", false).clicked() {
                        match ipc::install_hyprland_widget() {
                            Ok(m) => self.set_status(m.to_uppercase(), false),
                            Err(e) => self.set_status(e, true),
                        }
                    }
                    if pill_button(ui, "PLASMOID KDE 6", false).clicked() {
                        match ipc::install_kde_plasmoid() {
                            Ok(m) => self.set_status(m.to_uppercase(), false),
                            Err(e) => self.set_status(e, true),
                        }
                    }
                });
                if pill_button(ui, "ACTIVAR ACCESO USB DSP SIN SUDO (REGLAS UDEV)", false).clicked() {
                    match ipc::install_udev_rules() {
                        Ok(m) => {
                            self.set_status(m.to_uppercase(), false);
                            let id = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
                            self.spawn_refresh(ctx.clone(), id, false);
                        }
                        Err(e) => self.set_status(e, true),
                    }
                }
            });
        }

        ui.add_space(6.0);

        // ── Diagnostics Card ──
        sub_card_frame().show(ui, |ui| {
            section_label(ui, "DIAGNÓSTICO DEL MOTOR NATIVO RUST");
            let dev_line = self
                .active_device()
                .map(|d| format!("Activo: {} [{}] ({}B)", d.display_name(), d.badge(), d.max_bands()))
                .unwrap_or_else(|| "Activo: Ninguno".to_string());
            let diag = format!(
                "Motor: AutoQ v{} (100% Rust Nativo · Sin Python)\n{}\nDispositivos detectados: {} | PipeWire EQ: {}\nProtocolos HID: KTMicro 5B · Moondrop 8B · WalkPlay · FiiO 5/10B",
                env!("CARGO_PKG_VERSION"),
                dev_line,
                self.devices.len(),
                if self.pipewire_active { "ACTIVO" } else { "INACTIVO" }
            );
            ui.label(egui::RichText::new(diag).size(10.0).monospace().color(MUTED_GRAY));
        });
    }
}

impl eframe::App for AutoQApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_channels(ctx);

        egui::TopBottomPanel::bottom("telemetry_footer")
            .frame(
                egui::Frame::NONE
                    .fill(OBSIDIAN)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                let col = if self.status_error { NOTHING_RED } else { MUTED_GRAY };
                let prefix = if self.busy { "● " } else { "> " };
                ui.label(
                    egui::RichText::new(format!("{prefix}{}", self.status_text))
                        .size(10.0)
                        .monospace()
                        .color(col),
                );
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(OBSIDIAN)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                if self.full_mode {
                    self.render_full_app(ui, ctx);
                } else {
                    self.render_compact_widget(ui, ctx);
                }
            });
    }
}

fn print_waybar_status() {
    let devices = discover_devices();
    let active = pick_device_idx(&devices, "").and_then(|i| devices.get(i));
    let out = match active {
        Some(dev) => {
            let on = match dev {
                RuntimeDevice::UsbHid(info) => hid::read_device_eq(Some(info))
                    .map(|(_, en, _)| en)
                    .unwrap_or(true),
                RuntimeDevice::PipeWire(_) => platform_linux::is_pipewire_eq_running(),
                RuntimeDevice::WinApo { .. } => platform_windows::load_win_virtual_state().enabled,
            };
            let dot = if on { "●" } else { "○" };
            serde_json::json!({
                "text": format!("{dot} {}", dev.badge()),
                "tooltip": format!("AutoQ (Rust) — {}\nClick: abrir widget Nothing OS", dev.display_name()),
                "class": if on { "active" } else { "bypass" },
            })
        }
        None => serde_json::json!({
            "text": "○ AUTOQ",
            "tooltip": "AutoQ (Rust) — Sin dispositivo",
            "class": "offline",
        }),
    };
    println!("{out}");
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "--install-hyprland-widget") {
        match ipc::install_hyprland_widget() {
            Ok(m) => {
                println!("[OK] {m}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("[ERROR] {e}");
                std::process::exit(1);
            }
        }
    }
    if args.iter().any(|a| a == "--install-kde-plasmoid") {
        match ipc::install_kde_plasmoid() {
            Ok(m) => {
                println!("[OK] {m}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("[ERROR] {e}");
                std::process::exit(1);
            }
        }
    }
    if args.iter().any(|a| a == "--status-waybar") {
        print_waybar_status();
        return Ok(());
    }

    let start_full = !args.iter().any(|a| a == "--widget");
    let ipc_cmd = if args.iter().any(|a| a == "--toggle") {
        "toggle"
    } else if start_full {
        "full"
    } else {
        "widget"
    };

    if ipc::send_ipc_if_running(ipc_cmd) {
        return Ok(());
    }

    let (ipc_tx, ipc_rx) = mpsc::channel();
    ipc::start_ipc_listener(ipc_tx);

    let init_size = if start_full {
        [500.0, 720.0]
    } else {
        [390.0, 365.0]
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AutoQ — Universal Parametric EQ (Rust)")
            .with_app_id("autoq-gui")
            .with_inner_size(init_size)
            .with_min_inner_size([360.0, 330.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AutoQ",
        options,
        Box::new(move |cc| Ok(Box::new(AutoQApp::new(cc, start_full, ipc_rx)))),
    )
}
