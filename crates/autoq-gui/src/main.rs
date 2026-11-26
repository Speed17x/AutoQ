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
use autoq_core::biquad::{
    fit_band_count, flat_bands, FilterBand, FilterType, MAX_FREQ_HZ, MAX_Q, MIN_FREQ_HZ, MIN_Q,
};
use autoq_core::config::{load_presets, save_presets, DEFAULT_VIRTUAL_BANDS};
use autoq_core::hid::{self, MoondropDeviceInfo};
use autoq_core::platform_linux::{self, PipeWireAudioDevice, PW_SINK_NAME};
use autoq_core::platform_windows;
use eframe::egui::{self, CornerRadius, Stroke, StrokeKind, Vec2};
use graph::{draw_peq_graph, GraphEvent, GraphState};
use std::collections::BTreeMap;
use std::env;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use theme::{
    apply_nothing_theme, band_card, circle_icon_btn, draw_dot_matrix, inset_list_frame,
    nothing_slider, outline_pill, red_pill, section_label, segment_tab, sub_card_frame, BORDER,
    INSET_BG, MUTED_GRAY, NOTHING_RED, OBSIDIAN, PURE_WHITE, SURFACE_PILL,
};

const BRAND_CHIPS: &[(&str, &str)] = &[
    ("Todos", ""),
    ("7Hz", "7hz"),
    ("Moondrop", "moondrop"),
    ("Tanchjim", "tanchjim"),
    ("FiiO", "fiio"),
    ("Sony", "sony"),
    ("Huawei", "huawei"),
    ("Nothing", "nothing"),
    ("Apple", "apple"),
    ("Samsung", "samsung"),
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

    fn short_dot_title(&self) -> String {
        let up = self.display_name().to_uppercase();
        let filtered: String = up
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-')
            .collect();
        filtered.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(12).collect()
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
    full_mode: bool,
    active_tab: ActiveTab,
    ipc_rx: Receiver<String>,
    bg_tx: Sender<BgMsg>,
    bg_rx: Receiver<BgMsg>,

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

    stock_fr: Vec<[f64; 2]>,
    target_fr: Vec<[f64; 2]>,
    target_delta: Vec<[f64; 2]>,
    target_label: String,
    rmse_text: String,

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

    presets: BTreeMap<String, Vec<FilterBand>>,
    selected_preset: String,
    new_preset_name: String,
    apo_text: String,

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
            target_curve_idx: 1,
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
            Vec2::new(485.0, 845.0)
        } else {
            Vec2::new(390.0, 240.0)
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
                    match compute_target_curve_peq(
                        &src_entry,
                        csv_opt,
                        max_freq,
                        max_bands,
                        max_gain,
                        shelves,
                    ) {
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
                    match compute_match_headphone_peq(
                        &src_entry,
                        &tgt_entry,
                        max_freq,
                        max_bands,
                        max_gain,
                        shelves,
                    ) {
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

    fn render_compact_widget(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Header: ● SHORT_TITLE  [BADGE]  [ON]
        ui.horizontal(|ui| {
            let dot_title = self
                .active_device()
                .map(RuntimeDevice::short_dot_title)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "AUTOQ".to_string());
            draw_dot_matrix(ui, &dot_title, 2.3, PURE_WHITE, true);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let on_lbl = if self.dsp_enabled { "ON" } else { "OFF" };
                let on_resp = if self.dsp_enabled {
                    red_pill(ui, on_lbl, Vec2::new(42.0, 26.0))
                } else {
                    outline_pill(ui, on_lbl, false, Vec2::new(42.0, 26.0))
                };
                if on_resp.clicked() {
                    self.dsp_enabled = !self.dsp_enabled;
                    self.spawn_apply(ctx.clone(), false);
                }

                if let Some(dev) = self.active_device() {
                    egui::Frame::NONE
                        .fill(SURFACE_PILL)
                        .stroke(Stroke::new(1.0_f32, BORDER))
                        .corner_radius(CornerRadius::same(13))
                        .inner_margin(egui::Margin::symmetric(10, 5))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(dev.badge())
                                    .size(9.5)
                                    .strong()
                                    .monospace()
                                    .color(MUTED_GRAY),
                            );
                        });
                }
            });
        });

        ui.add_space(6.0);

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
                height: 130.0,
            },
            &mut self.dragging_node,
        );
        self.handle_graph_event(ev);

        ui.add_space(6.0);

        // Footer: (<)  [   Preset Name   ]  (>)  [ EQ ]
        let total_w = ui.available_width();
        let pill_w = (total_w - 30.0 - 30.0 - 48.0 - 21.0).max(120.0);
        ui.horizontal(|ui| {
            if circle_icon_btn(ui, "<", 30.0).clicked() {
                self.cycle_preset(ctx, -1);
            }

            let (pill_rect, _) = ui.allocate_exact_size(Vec2::new(pill_w, 30.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect(
                pill_rect,
                CornerRadius::same(15),
                INSET_BG,
                Stroke::new(1.0_f32, BORDER),
                StrokeKind::Inside,
            );
            painter.text(
                pill_rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.selected_preset,
                egui::FontId::monospace(11.0),
                PURE_WHITE,
            );

            if circle_icon_btn(ui, ">", 30.0).clicked() {
                self.cycle_preset(ctx, 1);
            }

            if red_pill(ui, "EQ", Vec2::new(48.0, 30.0)).clicked() {
                self.set_window_mode(ctx, true);
            }
        });
    }

    fn render_full_app(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // ── Top Header: ● AUTOQ  [Device Dropdown]  [ON]  (▾)  (×) ──
        ui.horizontal(|ui| {
            draw_dot_matrix(ui, "AUTOQ", 2.5, PURE_WHITE, true);

            let right_controls_w = 46.0 + 26.0 + 26.0 + 28.0;
            let combo_w = (ui.available_width() - right_controls_w).max(115.0);

            let cur_label = self
                .active_device()
                .map(|d| {
                    let name = d.display_name();
                    if name.chars().count() > 20 {
                        format!("{}…", name.chars().take(19).collect::<String>())
                    } else {
                        name
                    }
                })
                .unwrap_or_else(|| "Sin dispositivo".to_string());
            let mut chosen_idx = self.active_idx;
            egui::ComboBox::from_id_salt("device_selector")
                .width(combo_w)
                .truncate()
                .selected_text(
                    egui::RichText::new(cur_label)
                        .size(10.5)
                        .monospace()
                        .color(PURE_WHITE),
                )
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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if circle_icon_btn(ui, "×", 26.0)
                    .on_hover_text("Minimizar ventana")
                    .clicked()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                if circle_icon_btn(ui, "▾", 26.0)
                    .on_hover_text("Modo widget compacto")
                    .clicked()
                {
                    self.set_window_mode(ctx, false);
                }
                let on_lbl = if self.dsp_enabled { "ON" } else { "OFF" };
                let on_resp = if self.dsp_enabled {
                    red_pill(ui, on_lbl, Vec2::new(42.0, 26.0))
                } else {
                    outline_pill(ui, on_lbl, false, Vec2::new(42.0, 26.0))
                };
                if on_resp.clicked() {
                    self.dsp_enabled = !self.dsp_enabled;
                    self.spawn_apply(ctx.clone(), false);
                }
            });
        });

        ui.add_space(4.0);

        // ── 3-Tab Nothing OS Segmented Pill Bar ──
        egui::Frame::NONE
            .fill(INSET_BG)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CornerRadius::same(18))
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                let inner_w = ui.available_width();
                let seg_w = ((inner_w - 14.0) / 3.0).max(80.0);
                ui.horizontal(|ui| {
                    if segment_tab(ui, "EQ", self.active_tab == ActiveTab::Eq, seg_w).clicked() {
                        self.active_tab = ActiveTab::Eq;
                    }
                    if segment_tab(ui, "AUTOQ", self.active_tab == ActiveTab::AutoQ, seg_w).clicked() {
                        self.active_tab = ActiveTab::AutoQ;
                    }
                    if segment_tab(ui, "AVANZADO", self.active_tab == ActiveTab::Advanced, seg_w).clicked() {
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
        // Quick action strip: [FR] [0 dB] [↻ Leer] [LIVE]      [GUARDAR FLASH / APLICAR EQ]
        ui.horizontal(|ui| {
            let fr_lbl = if self.overlay_fr { "FR" } else { "ΔEQ" };
            if outline_pill(ui, fr_lbl, self.overlay_fr, Vec2::new(42.0, 28.0)).clicked() {
                self.overlay_fr = !self.overlay_fr;
            }
            if outline_pill(ui, "0 dB", false, Vec2::new(50.0, 28.0)).clicked() {
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
            if outline_pill(ui, "↻ Leer", false, Vec2::new(66.0, 28.0)).clicked() {
                let id = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
                self.spawn_refresh(ctx.clone(), id, false);
            }
            if outline_pill(ui, "LIVE", self.live_preview, Vec2::new(50.0, 28.0)).clicked() {
                self.live_preview = !self.live_preview;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_usb = self.active_device().map(RuntimeDevice::is_usb_hid).unwrap_or(false);
                let cta = if is_usb {
                    if self.dirty_bands { "GUARDAR FLASH ●" } else { "GUARDAR FLASH" }
                } else if self.dirty_bands {
                    "APLICAR PW *"
                } else {
                    "APLICAR EQ"
                };
                if red_pill(ui, cta, Vec2::new(118.0, 30.0)).clicked() {
                    self.spawn_apply(ctx.clone(), is_usb);
                }
            });
        });

        ui.add_space(5.0);

        // Interactive Graph (205px)
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
                height: 205.0,
            },
            &mut self.dragging_node,
        );
        self.handle_graph_event(ev);

        ui.add_space(6.0);

        // Structured 5-column × 2-row Band Cards Grid
        let n_bands = self.bands.len();
        let avail_w = ui.available_width();
        let gap = 6.0;
        let card_w = ((avail_w - gap * 4.0) / 5.0).max(56.0);
        for row_start in (0..n_bands).step_by(5) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for idx in row_start..(row_start + 5).min(n_bands) {
                    let b = &self.bands[idx];
                    if band_card(ui, idx, b.frequency, b.gain, idx == self.selected_band, card_w)
                        .clicked()
                    {
                        self.selected_band = idx;
                    }
                }
            });
        }

        ui.add_space(6.0);

        // Selected Band Fine-Tuning Card
        let supports_shelves = self
            .active_device()
            .map(RuntimeDevice::supports_shelves)
            .unwrap_or(true);
        let mut changed = false;

        sub_card_frame().show(ui, |ui| {
            let idx = self.selected_band.min(self.bands.len().saturating_sub(1));
            let total = self.bands.len();
            if let Some(b) = self.bands.get_mut(idx) {
                ui.horizontal(|ui| {
                    section_label(ui, &format!("BANDA  {} / {}", idx + 1, total));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if outline_pill(ui, "0.0", false, Vec2::new(40.0, 24.0)).clicked() {
                            b.gain = 0.0;
                            changed = true;
                        }
                        if supports_shelves {
                            if outline_pill(
                                ui,
                                "HS",
                                b.filter_type == FilterType::HighShelf,
                                Vec2::new(36.0, 24.0),
                            )
                            .clicked()
                            {
                                b.filter_type = FilterType::HighShelf;
                                changed = true;
                            }
                            if outline_pill(
                                ui,
                                "LS",
                                b.filter_type == FilterType::LowShelf,
                                Vec2::new(36.0, 24.0),
                            )
                            .clicked()
                            {
                                b.filter_type = FilterType::LowShelf;
                                changed = true;
                            }
                        }
                        if outline_pill(
                            ui,
                            "PK",
                            b.filter_type == FilterType::Peaking,
                            Vec2::new(36.0, 24.0),
                        )
                        .clicked()
                        {
                            b.filter_type = FilterType::Peaking;
                            changed = true;
                        }
                    });
                });

                ui.add_space(6.0);
                let f_txt = format!("{} Hz", b.frequency.round() as i64);
                if nothing_slider(
                    ui,
                    "FREQ",
                    &mut b.frequency,
                    MIN_FREQ_HZ,
                    MAX_FREQ_HZ,
                    0.04,
                    true,
                    &f_txt,
                ) {
                    b.frequency = b.frequency.round();
                    changed = true;
                }

                let g_txt = format!("{:+.1} dB", b.gain);
                if nothing_slider(ui, "GAIN", &mut b.gain, -max_g, max_g, 0.1, false, &g_txt) {
                    b.gain = (b.gain * 10.0).round() / 10.0;
                    changed = true;
                }

                let q_txt = format!("Q {:.2}", b.q);
                if nothing_slider(ui, "Q", &mut b.q, MIN_Q, MAX_Q, 0.05, true, &q_txt) {
                    b.q = (b.q * 100.0).round() / 100.0;
                    changed = true;
                }
            }
        });

        if changed {
            self.schedule_live_apply();
        }
    }

    fn render_autoq_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Catalog summary + Sync button
        ui.horizontal(|ui| {
            section_label(
                ui,
                &format!(
                    "CATÁLOGO: {} CURVAS · {}/{} FUENTES",
                    self.catalog.len(),
                    self.active_sources,
                    self.total_sources
                ),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if outline_pill(ui, "↻ Sync 8 Fuentes", false, Vec2::new(118.0, 26.0)).clicked() {
                    let id = self.active_device().map(RuntimeDevice::id).unwrap_or_default();
                    self.spawn_refresh(ctx.clone(), id, true);
                }
            });
        });

        ui.add_space(4.0);

        // Source headphone picker card
        sub_card_frame().show(ui, |ui| {
            let cur_src_lbl = self
                .selected_src_idx
                .and_then(|i| self.catalog.get(i))
                .map(|e| format!("TU AUDÍFONO: {} [{}]", e.name, e.source_rig))
                .unwrap_or_else(|| "TU AUDÍFONO: (Ninguno seleccionado)".to_string());
            ui.label(egui::RichText::new(cur_src_lbl).size(11.0).strong().color(PURE_WHITE));

            ui.add(
                egui::TextEdit::singleline(&mut self.src_query)
                    .hint_text("Buscar tu audífono (Golden Ages 2, Zero 3, Chu 2, XM5...)")
                    .desired_width(f32::INFINITY),
            );

            egui::ScrollArea::horizontal()
                .id_salt("brand_chips_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for &(lbl, tok) in BRAND_CHIPS {
                            if outline_pill(
                                ui,
                                lbl,
                                self.src_brand_filter == tok,
                                Vec2::new(0.0, 24.0),
                            )
                            .clicked()
                            {
                                self.src_brand_filter = tok.to_string();
                            }
                        }
                    });
                });

            let hits = self.filter_catalog_indices(&self.src_query, &self.src_brand_filter, 40);
            inset_list_frame().show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("src_results_scroll")
                    .max_height(105.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
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
        });

        ui.add_space(6.0);

        // 2-column mode switcher: CURVA TARGET | IMITAR AUDÍFONO
        let col_w = ((ui.available_width() - 8.0) / 2.0).max(120.0);
        ui.horizontal(|ui| {
            if outline_pill(
                ui,
                "CURVA TARGET",
                self.autoq_mode == AutoQMode::TargetCurve,
                Vec2::new(col_w, 30.0),
            )
            .clicked()
            {
                self.autoq_mode = AutoQMode::TargetCurve;
            }
            if outline_pill(
                ui,
                "IMITAR AUDÍFONO",
                self.autoq_mode == AutoQMode::MatchHeadphone,
                Vec2::new(col_w, 30.0),
            )
            .clicked()
            {
                self.autoq_mode = AutoQMode::MatchHeadphone;
            }
        });

        ui.add_space(4.0);

        // Target / Match card
        sub_card_frame().show(ui, |ui| match self.autoq_mode {
            AutoQMode::TargetCurve => {
                section_label(ui, "CURVA OBJETIVO (8 TARGETS VERIFICADOS)");
                ui.add_space(4.0);
                let cur_t = CURATED_TARGETS
                    .get(self.target_curve_idx)
                    .map(|&(l, _)| l)
                    .unwrap_or("AutoEq In-Ear (Predeterminado)");
                egui::ComboBox::from_id_salt("target_curve_combo")
                    .width(ui.available_width())
                    .selected_text(cur_t)
                    .show_ui(ui, |ui| {
                        for (i, &(label, _)) in CURATED_TARGETS.iter().enumerate() {
                            if ui.selectable_label(self.target_curve_idx == i, label).clicked() {
                                self.target_curve_idx = i;
                            }
                        }
                    });
            }
            AutoQMode::MatchHeadphone => {
                section_label(ui, "AUDÍFONO DESTINO A IMITAR");
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
                let hits =
                    self.filter_catalog_indices(&self.match_query, &self.match_brand_filter, 40);
                inset_list_frame().show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("match_results_scroll")
                        .max_height(100.0)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
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
                });
            }
        });

        ui.add_space(6.0);

        // TECHO row (≤6k .. ≤20k)
        ui.horizontal(|ui| {
            section_label(ui, "TECHO:");
            let chip_w = ((ui.available_width() - 32.0) / 5.0).max(46.0);
            for (hz, lbl) in [
                (6000.0, "≤6k"),
                (8000.0, "≤8k"),
                (10000.0, "≤10k"),
                (14000.0, "≤14k"),
                (20000.0, "≤20k"),
            ] {
                if outline_pill(
                    ui,
                    lbl,
                    (self.max_freq_fit - hz).abs() < 1.0,
                    Vec2::new(chip_w, 26.0),
                )
                .clicked()
                {
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
        if red_pill(ui, &btn_lbl, Vec2::new(ui.available_width(), 34.0)).clicked() {
            self.spawn_autoq(ctx.clone());
        }

        if !self.autoq_summary.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(&self.autoq_summary)
                    .size(10.5)
                    .strong()
                    .color(PURE_WHITE),
            );
        }
    }

    fn render_advanced_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // ── Presets Card ──
        sub_card_frame().show(ui, |ui| {
            section_label(ui, "PRESETS UNIVERSALES (SINCRONIZADOS CON WIDGET)");
            let keys: Vec<String> = self.presets.keys().cloned().collect();
            inset_list_frame().show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("presets_scroll")
                    .max_height(88.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        for k in &keys {
                            if ui.selectable_label(&self.selected_preset == k, k).clicked() {
                                self.selected_preset = k.clone();
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                let sel = self.selected_preset.clone();
                let apply_w = (ui.available_width() - 94.0).max(120.0);
                if red_pill(ui, "APLICAR", Vec2::new(apply_w, 28.0)).clicked() {
                    self.apply_preset_by_name(ctx, &sel);
                }
                if outline_pill(ui, "ELIMINAR", false, Vec2::new(86.0, 28.0)).clicked() {
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
                        .desired_width(ui.available_width() - 94.0),
                );
                if outline_pill(ui, "GUARDAR", false, Vec2::new(86.0, 26.0)).clicked() {
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
            inset_list_frame().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.apo_text)
                        .frame(false)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("Pega aquí Filter 1: ON PK Fc 100 Hz Gain 2.5 dB Q 1.2 ..."),
                );
            });
            let half_w = ((ui.available_width() - 8.0) / 2.0).max(110.0);
            ui.horizontal(|ui| {
                if outline_pill(ui, "IMPORTAR APO", false, Vec2::new(half_w, 28.0)).clicked() {
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
                        self.set_status(
                            format!("IMPORTADAS {} BANDAS APO", parsed.len().min(n)),
                            false,
                        );
                        if self.live_preview {
                            self.spawn_apply(ctx.clone(), false);
                        }
                    }
                }
                if outline_pill(ui, "EXPORTAR Y COPIAR", false, Vec2::new(half_w, 28.0)).clicked() {
                    let exported = export_parametric_eq(&self.bands);
                    self.apo_text = exported.clone();
                    ctx.copy_text(exported);
                    self.set_status("PARAMETRICEQ.TXT COPIADO AL PORTAPAPELES", false);
                }
            });
        });

        // ── Bluetooth SPP Firmware Card ──
        if let Some(mac) = self
            .active_device()
            .filter(|d| d.supports_spp())
            .and_then(|d| d.mac().map(str::to_string))
        {
            ui.add_space(6.0);
            sub_card_frame().show(ui, |ui| {
                section_label(ui, "BLUETOOTH SPP FIRMWARE (MOONDROP)");
                let third_w = ((ui.available_width() - 14.0) / 3.0).max(75.0);
                ui.horizontal(|ui| {
                    for (idx, lbl) in [(0u8, "FW: POP"), (1u8, "FW: MON"), (2u8, "FW: BASS")] {
                        if outline_pill(ui, lbl, false, Vec2::new(third_w, 26.0)).clicked() {
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
                let half_w = ((ui.available_width() - 8.0) / 2.0).max(110.0);
                ui.horizontal(|ui| {
                    if outline_pill(ui, "SPP LEER", false, Vec2::new(half_w, 28.0)).clicked() {
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
                    if red_pill(ui, "SPP ESCRIBIR", Vec2::new(half_w, 28.0)).clicked() {
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
                let half_w = ((ui.available_width() - 8.0) / 2.0).max(110.0);
                ui.horizontal(|ui| {
                    if outline_pill(ui, "WIDGET HYPRLAND", false, Vec2::new(half_w, 28.0)).clicked() {
                        match ipc::install_hyprland_widget() {
                            Ok(m) => self.set_status(m.to_uppercase(), false),
                            Err(e) => self.set_status(e, true),
                        }
                    }
                    if outline_pill(ui, "PLASMOID KDE 6", false, Vec2::new(half_w, 28.0)).clicked() {
                        match ipc::install_kde_plasmoid() {
                            Ok(m) => self.set_status(m.to_uppercase(), false),
                            Err(e) => self.set_status(e, true),
                        }
                    }
                });
                if outline_pill(
                    ui,
                    "ACTIVAR ACCESO USB DSP SIN SUDO (REGLAS UDEV)",
                    false,
                    Vec2::new(ui.available_width(), 28.0),
                )
                .clicked()
                {
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
            section_label(ui, "DIAGNÓSTICO DE AUDIO Y MOTOR RUST");
            let dev_line = self
                .active_device()
                .map(|d| {
                    format!(
                        "Dispositivo: {}\nTransporte: {} · Bandas: {} · Rango: ±{:.0} dB",
                        d.display_name(),
                        d.badge(),
                        d.max_bands(),
                        d.max_gain_db()
                    )
                })
                .unwrap_or_else(|| "Dispositivo: Ninguno conectado".to_string());
            let diag = format!(
                "{}\nPipeWire Filter-Chain: {}\nMotor: AutoQ v{} (100% Rust Nativo · Sin Python)\nProtocolos HID: KTMicro 5B · Moondrop 8B · WalkPlay · FiiO 5/10B",
                dev_line,
                if self.pipewire_active { "ACTIVO" } else { "INACTIVO" },
                env!("CARGO_PKG_VERSION"),
            );
            ui.label(
                egui::RichText::new(diag)
                    .size(10.0)
                    .monospace()
                    .color(MUTED_GRAY),
            );
        });
    }
}

impl eframe::App for AutoQApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_channels(ctx);

        if self.full_mode {
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
        }

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
        [485.0, 845.0]
    } else {
        [390.0, 240.0]
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AutoQ — Universal Parametric EQ (Rust)")
            .with_app_id("autoq-gui")
            .with_inner_size(init_size)
            .with_min_inner_size([360.0, 230.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AutoQ",
        options,
        Box::new(move |cc| Ok(Box::new(AutoQApp::new(cc, start_full, ipc_rx)))),
    )
}
