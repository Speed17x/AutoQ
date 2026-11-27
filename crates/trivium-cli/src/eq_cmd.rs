//! EQ apply/flat, AutoEQ catalog/matching, presets, exporters, and Bluetooth SPP commands.

use crate::devices::{discover_all_devices, pick_device, RuntimeDevice};
use crate::CliArgs;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use trivium_core::autoeq::{
    compute_match_headphone_peq, compute_target_curve_peq, downsample_curve, export_camilladsp_yaml,
    export_easyeffects_json, export_parametric_eq, export_parametric_eq_with_preamp,
    export_wavelet_graphiceq, find_best_entry_any_brand, load_catalog, normalize_search_text,
    parse_parametric_eq, reconstruct_fr_curve, AutoEqEntry, CURATED_TARGETS,
};
use trivium_core::biquad::{compute_safe_preamp_db, fit_band_count, flat_bands, FilterBand};
use trivium_core::config::{load_presets, save_presets, DEFAULT_VIRTUAL_BANDS};
use trivium_core::hid;
use trivium_core::platform_linux;
use trivium_core::platform_windows;

pub(crate) fn entry_to_json(e: &AutoEqEntry) -> Value {
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

pub(crate) fn decode_b64(val: &str) -> Result<String, String> {
    if val.is_empty() {
        return Ok(String::new());
    }
    let bytes = B64
        .decode(val.trim().as_bytes())
        .map_err(|e| format!("Base64 inválido: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 inválido en Base64: {e}"))
}

pub(crate) fn parse_bands_json(raw: &str) -> Result<Vec<FilterBand>, String> {
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

pub(crate) fn cmd_apply(
    device_id: &str,
    bands_json: &str,
    commit: bool,
    enable_eq: bool,
    preamp_db: Option<f64>,
    output_gain_db: f64,
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
            let active = platform_linux::apply_pipewire_eq_with_gains(
                pw,
                &fitted,
                enable_eq,
                preamp_db,
                output_gain_db,
            )?;
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
            let auto_preamp = preamp_db.is_none();
            let path = platform_windows::apply_apo_eq_with_gains(
                &fitted,
                enable_eq,
                preamp_db,
                auto_preamp,
                output_gain_db,
            )?;
            let fname = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "trivium_eq.txt".to_string());
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

pub(crate) fn cmd_flat(device_id: &str, commit: bool) -> Result<Value, String> {
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
    cmd_apply(&dev.id(), &raw, commit, true, None, 0.0)
}

pub(crate) fn cmd_search(
    query: &str,
    moondrop_only: bool,
    in_ear_only: bool,
    limit: usize,
) -> Value {
    let (entries, _, _) = load_catalog(false);
    let norm_q = normalize_search_text(query);
    let tokens: Vec<&str> = norm_q.split_whitespace().collect();

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

pub(crate) fn cmd_source_fr(src_key: &str) -> Result<Value, String> {
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

pub(crate) fn cmd_autoeq_match(
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

pub(crate) fn cmd_autoeq_target(
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

pub(crate) fn cmd_preset_save(name: String, bands_json: &str) -> Result<Value, String> {
    let mut presets = load_presets();
    presets.insert(name.clone(), parse_bands_json(bands_json)?);
    save_presets(&presets)?;
    Ok(json!({
        "ok": true,
        "presets": presets,
        "message": format!("Preset '{name}' guardado."),
    }))
}

pub(crate) fn cmd_preset_delete(name: &str) -> Result<Value, String> {
    let mut presets = load_presets();
    presets.remove(name);
    save_presets(&presets)?;
    Ok(json!({
        "ok": true,
        "presets": presets,
        "message": format!("Preset '{name}' eliminado."),
    }))
}

pub(crate) fn cmd_import_apo(text: &str, max_bands: usize) -> Value {
    let parsed = parse_parametric_eq(text);
    if parsed.is_empty() {
        return json!({
            "ok": false,
            "error": "No se encontraron líneas 'Filter N: ON PK/LSC/HSC ...' válidas."
        });
    }
    let count = parsed.len().min(max_bands);
    let fitted = fit_band_count(&parsed, max_bands);
    json!({
        "ok": true,
        "bands": fitted,
        "message": format!("Importadas {count} bandas APO."),
    })
}

pub(crate) fn cmd_export_format(
    fmt: &str,
    bands_json: &str,
    preamp_opt: Option<f64>,
) -> Result<Value, String> {
    let bands = parse_bands_json(bands_json)?;
    let preamp = preamp_opt.unwrap_or_else(|| compute_safe_preamp_db(&bands));
    let text = match fmt {
        "easyeffects" | "ee" | "json" => export_easyeffects_json(&bands, preamp),
        "camilladsp" | "camilla" | "yaml" | "yml" => export_camilladsp_yaml(&bands, preamp),
        "wavelet" | "graphiceq" => export_wavelet_graphiceq(&bands),
        _ => export_parametric_eq_with_preamp(&bands, preamp),
    };
    Ok(json!({
        "ok": true,
        "format": fmt,
        "text": text,
    }))
}

pub(crate) fn cmd_export_apo(bands_json: &str, preamp_opt: Option<f64>) -> Result<Value, String> {
    let bands = parse_bands_json(bands_json)?;
    let text = match preamp_opt {
        Some(p) => export_parametric_eq_with_preamp(&bands, p),
        None => export_parametric_eq(&bands),
    };
    Ok(json!({
        "ok": true,
        "text": text,
    }))
}

pub(crate) fn cmd_bt_spp(
    mac: &str,
    action: &str,
    preset_idx: u8,
    bands_json: &str,
) -> Result<Value, String> {
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

fn source_credit_url(e: &AutoEqEntry) -> String {
    if e.source_kind == "squiglink" && !e.squig_base_url.is_empty() {
        e.squig_base_url.trim_end_matches("data/").to_string()
    } else {
        "https://github.com/jaakkopasanen/AutoEq (MIT)".to_string()
    }
}

pub(crate) fn run_human_eq(args: &CliArgs) -> Result<(), String> {
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
            "Indica tu modelo de audífono, p. ej.: trivium-cli eq \"Moondrop Chu 2\" --target \"Harman In-Ear 2019\" [--apply] [--commit] [--out eq.txt]"
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
    println!("  Trivium CLI (Rust) — Calibración Paramétrica Bajo Demanda");
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
        let mut fmt = args.str_or("--format", "").to_ascii_lowercase();
        if fmt.is_empty() {
            let low = out_path.to_ascii_lowercase();
            fmt = if low.ends_with(".json") {
                "easyeffects".to_string()
            } else if low.ends_with(".yaml") || low.ends_with(".yml") {
                "camilladsp".to_string()
            } else {
                "apo".to_string()
            };
        }
        let safe_preamp = compute_safe_preamp_db(&m.bands);
        let body = match fmt.as_str() {
            "easyeffects" | "ee" | "json" => export_easyeffects_json(&m.bands, safe_preamp),
            "camilladsp" | "camilla" | "yaml" | "yml" => {
                export_camilladsp_yaml(&m.bands, safe_preamp)
            }
            "wavelet" | "graphiceq" => export_wavelet_graphiceq(&m.bands),
            _ => {
                let mut txt = format!(
                    "# Generado por Trivium CLI (Rust)\n# Origen: {} — Medición cortesía de {} ({})\n# Objetivo: {} (RMSE {:.2} dB)\n",
                    src.name,
                    src.source_rig,
                    source_credit_url(src),
                    target_desc,
                    m.rmse
                );
                txt.push_str(&export_parametric_eq_with_preamp(&m.bands, safe_preamp));
                txt
            }
        };
        std::fs::write(&out_path, body)
            .map_err(|e| format!("Error guardando '{out_path}': {e}"))?;
        println!("  [OK] Exportado ({fmt}) en: {out_path}");
    }

    let do_commit = args.has("--commit");
    let do_apply = args.has("--apply") || do_commit;
    if do_apply {
        let raw_bands = serde_json::to_string(&m.bands).map_err(|e| e.to_string())?;
        let res = cmd_apply(
            &dev_id,
            &raw_bands,
            do_commit,
            true,
            None,
            args.f64_or("--output-gain", 0.0),
        )?;
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
