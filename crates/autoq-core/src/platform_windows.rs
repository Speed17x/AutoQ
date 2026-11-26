//! Windows 10/11 system-wide 10-band EQ backend via Equalizer APO (`autoq_eq.txt`).

use crate::biquad::{compute_safe_preamp_db, flat_bands, FilterBand};
use crate::config::{win_state_file, DEFAULT_VIRTUAL_BANDS};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

pub const AUTOQ_APO_FILENAME: &str = "autoq_eq.txt";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinVirtualState {
    pub enabled: bool,
    pub bands: Vec<FilterBand>,
    #[serde(default)]
    pub preamp_db: f64,
    #[serde(default = "default_true")]
    pub auto_preamp: bool,
}

fn default_true() -> bool {
    true
}

pub fn load_win_virtual_state() -> WinVirtualState {
    let path = win_state_file();
    if let Ok(txt) = fs::read_to_string(path) {
        if let Ok(st) = serde_json::from_str::<WinVirtualState>(&txt) {
            return st;
        }
    }
    WinVirtualState {
        enabled: true,
        bands: flat_bands(DEFAULT_VIRTUAL_BANDS),
        preamp_db: 0.0,
        auto_preamp: true,
    }
}

pub fn save_win_virtual_state(enabled: bool, bands: &[FilterBand]) {
    save_win_virtual_state_full(enabled, bands, 0.0, true);
}

pub fn save_win_virtual_state_full(
    enabled: bool,
    bands: &[FilterBand],
    preamp_db: f64,
    auto_preamp: bool,
) {
    let path = win_state_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let st = WinVirtualState {
        enabled,
        bands: bands.iter().map(FilterBand::clamped).collect(),
        preamp_db: if preamp_db.is_finite() {
            (preamp_db.clamp(-18.0, 6.0) * 100.0).round() / 100.0
        } else {
            0.0
        },
        auto_preamp,
    };
    if let Ok(json) = serde_json::to_string_pretty(&st) {
        let _ = fs::write(path, json);
    }
}

pub fn find_apo_config_dir() -> Option<PathBuf> {
    if let Ok(override_dir) = env::var("AUTOQ_APO_CONFIG_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_dir() {
                return Some(p);
            }
        }
    }

    let pf = env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string());
    let pfx86 =
        env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".to_string());

    for base in [pf, pfx86] {
        let candidate = PathBuf::from(base).join("EqualizerAPO").join("config");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

pub fn format_apo_config(bands: &[FilterBand], enabled: bool) -> String {
    format_apo_config_with_preamp(bands, enabled, None)
}

pub fn format_apo_config_with_preamp(
    bands: &[FilterBand],
    enabled: bool,
    preamp_db: Option<f64>,
) -> String {
    if !enabled {
        return "# AutoQ — Bypass activo\nPreamp: 0.0 dB\n".to_string();
    }
    let preamp = preamp_db
        .filter(|v| v.is_finite())
        .map(|v| (v.clamp(-18.0, 6.0) * 100.0).round() / 100.0)
        .unwrap_or_else(|| compute_safe_preamp_db(bands));
    let mut lines = Vec::with_capacity(bands.len() + 2);
    lines.push("# Generado automáticamente por AutoQ (Rust Core)".to_string());
    lines.push(format!("Preamp: {preamp:.2} dB"));
    for (idx, b) in bands.iter().enumerate() {
        lines.push(format!(
            "Filter {}: ON {} Fc {:.1} Hz Gain {:.2} dB Q {:.3}",
            idx + 1,
            b.filter_type.apo_code(),
            b.frequency,
            b.gain,
            b.q
        ));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn apply_apo_eq(bands: &[FilterBand], enabled: bool) -> Result<PathBuf, String> {
    apply_apo_eq_with_preamp(bands, enabled, None, true)
}

pub fn apply_apo_eq_with_preamp(
    bands: &[FilterBand],
    enabled: bool,
    preamp_db: Option<f64>,
    auto_preamp: bool,
) -> Result<PathBuf, String> {
    let stored_preamp = preamp_db.unwrap_or_else(|| compute_safe_preamp_db(bands));
    save_win_virtual_state_full(enabled, bands, stored_preamp, auto_preamp);
    let cfg_dir = find_apo_config_dir().ok_or_else(|| {
        "No se encontró Equalizer APO en 'C:\\Program Files\\EqualizerAPO\\config'. Instala Equalizer APO para usar EQ global de 10 bandas en Windows.".to_string()
    })?;

    let target_file = cfg_dir.join(AUTOQ_APO_FILENAME);
    let main_cfg = cfg_dir.join("config.txt");

    fs::write(
        &target_file,
        format_apo_config_with_preamp(bands, enabled, preamp_db),
    )
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "Sin permisos para escribir en {}. Ejecuta AutoQ como Administrador o otorga permisos de escritura a la carpeta config.",
                cfg_dir.display()
            )
        } else {
            format!("Error escribiendo Equalizer APO: {e}")
        }
    })?;

    let include_line = format!("Include: {AUTOQ_APO_FILENAME}");
    if main_cfg.exists() {
        let txt = fs::read_to_string(&main_cfg).unwrap_or_default();
        if !txt.contains(&include_line) {
            let sep = if txt.is_empty() || txt.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let new_txt = format!("{txt}{sep}{include_line}\n");
            fs::write(&main_cfg, new_txt).map_err(|e| format!("Error actualizando config.txt: {e}"))?;
        }
    } else {
        fs::write(&main_cfg, format!("{include_line}\n"))
            .map_err(|e| format!("Error creando config.txt: {e}"))?;
    }

    Ok(target_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biquad::FilterType;

    #[test]
    fn formats_apo_config_with_negative_preamp_for_boosts() {
        let bands = vec![FilterBand::new(FilterType::Peaking, 1000.0, 5.0, 1.0)];
        let txt = format_apo_config(&bands, true);
        assert!(txt.contains("Preamp: -5."));
        assert!(txt.contains("Filter 1: ON PK Fc 1000.0 Hz Gain 5.00 dB Q 1.000"));
    }
}
