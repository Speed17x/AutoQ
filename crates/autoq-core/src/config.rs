//! Configuration, presets, and cache path management with seamless migration
//! from `~/.config/chu2-dsp-widget` and `~/.cache/chu2-dsp-widget` to `autoq`.

use crate::biquad::{flat_bands, FilterBand, FilterType};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_VIRTUAL_BANDS: usize = 10;

fn home_dir() -> PathBuf {
    if let Ok(h) = env::var("HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    if let Ok(up) = env::var("USERPROFILE") {
        if !up.is_empty() {
            return PathBuf::from(up);
        }
    }
    PathBuf::from(".")
}

pub fn config_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"))
    } else {
        env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"))
    };

    let primary = if cfg!(target_os = "windows") {
        base.join("AutoQ")
    } else {
        base.join("autoq")
    };

    // One-time transparent migration from legacy ~/.config/chu2-dsp-widget
    if !cfg!(target_os = "windows") {
        let legacy = base.join("chu2-dsp-widget");
        if legacy.is_dir() && !primary.exists() {
            let _ = fs::create_dir_all(&primary);
            for fname in ["presets.json", "bt_profiles.json", "pw-moondrop-eq.conf"] {
                let src = legacy.join(fname);
                let dst = primary.join(fname);
                if src.is_file() && !dst.exists() {
                    let _ = fs::copy(&src, &dst);
                }
            }
        }
    }

    primary
}

pub fn cache_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".cache"))
    } else {
        env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".cache"))
    };

    let primary = if cfg!(target_os = "windows") {
        base.join("AutoQ").join("cache")
    } else {
        base.join("autoq")
    };

    // Reuse existing catalog/curves from ~/.cache/chu2-dsp-widget if primary doesn't have catalog yet
    if !cfg!(target_os = "windows") && !primary.join("catalog_v2.json").exists() {
        let legacy = base.join("chu2-dsp-widget");
        if legacy.join("catalog_v2.json").is_file() {
            return legacy;
        }
    }

    primary
}

pub fn presets_file() -> PathBuf {
    config_dir().join("presets.json")
}

pub fn bt_profiles_file() -> PathBuf {
    config_dir().join("bt_profiles.json")
}

pub fn win_state_file() -> PathBuf {
    config_dir().join("win_eq_state.json")
}

pub fn default_presets() -> BTreeMap<String, Vec<FilterBand>> {
    let mut map = BTreeMap::new();
    map.insert("Plano (0 dB)".to_string(), flat_bands(DEFAULT_VIRTUAL_BANDS));
    map.insert(
        "Harman Warm".to_string(),
        vec![
            FilterBand::new(FilterType::LowShelf, 105.0, 3.2, 0.707),
            FilterBand::new(FilterType::Peaking, 240.0, -1.4, 1.100),
            FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.000),
            FilterBand::new(FilterType::Peaking, 2800.0, 1.8, 1.400),
            FilterBand::new(FilterType::Peaking, 6200.0, -1.6, 3.200),
            FilterBand::new(FilterType::HighShelf, 9500.0, 1.2, 0.707),
        ],
    );
    map.insert(
        "Vocal / Ref".to_string(),
        vec![
            FilterBand::new(FilterType::LowShelf, 85.0, -1.5, 0.707),
            FilterBand::new(FilterType::Peaking, 320.0, -1.0, 1.200),
            FilterBand::new(FilterType::Peaking, 1600.0, 2.2, 1.300),
            FilterBand::new(FilterType::Peaking, 3600.0, 1.5, 1.800),
            FilterBand::new(FilterType::HighShelf, 10000.0, 0.8, 0.707),
        ],
    );
    map.insert(
        "Bass Boost".to_string(),
        vec![
            FilterBand::new(FilterType::LowShelf, 90.0, 4.5, 0.707),
            FilterBand::new(FilterType::Peaking, 220.0, -1.0, 1.200),
            FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.000),
            FilterBand::new(FilterType::Peaking, 3200.0, 0.8, 1.400),
            FilterBand::new(FilterType::HighShelf, 10000.0, 0.5, 0.707),
        ],
    );
    map
}

pub fn load_presets() -> BTreeMap<String, Vec<FilterBand>> {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("presets.json");
    let defaults = default_presets();

    if !path.exists() {
        let _ = save_presets(&defaults);
        return defaults;
    }

    if let Ok(txt) = fs::read_to_string(&path) {
        if let Ok(mut data) = serde_json::from_str::<BTreeMap<String, Vec<FilterBand>>>(&txt) {
            let mut changed = false;
            for old_key in [
                "Chu 2 DSP (Fábrica)",
                "Plano / Stock sin EQ (0 dB)",
                "Plano / 3.5mm Stock (0 dB)",
            ] {
                if data.remove(old_key).is_some() {
                    changed = true;
                }
            }
            for (k, v) in &defaults {
                if !data.contains_key(k) {
                    data.insert(k.clone(), v.clone());
                    changed = true;
                }
            }
            if changed {
                let _ = save_presets(&data);
            }
            return data;
        }
    }
    defaults
}

pub fn save_presets(data: &BTreeMap<String, Vec<FilterBand>>) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("presets.json");
    let normalized: BTreeMap<String, Vec<FilterBand>> = data
        .iter()
        .map(|(k, bands)| (k.clone(), bands.iter().map(FilterBand::clamped).collect()))
        .collect();
    let json = serde_json::to_string_pretty(&normalized).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn load_saved_profile_bands(path: &Path, key: &str, count: usize) -> Vec<FilterBand> {
    let defaults = [31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
    if let Ok(txt) = fs::read_to_string(path) {
        if let Ok(raw) = serde_json::from_str::<BTreeMap<String, Vec<FilterBand>>>(&txt) {
            if let Some(items) = raw.get(key) {
                if !items.is_empty() {
                    let mut bands: Vec<FilterBand> =
                        items.iter().take(count).map(FilterBand::clamped).collect();
                    while bands.len() < count {
                        bands.push(FilterBand::new(
                            FilterType::Peaking,
                            defaults[bands.len() % defaults.len()],
                            0.0,
                            1.0,
                        ));
                    }
                    return bands;
                }
            }
        }
    }
    (0..count)
        .map(|i| FilterBand::new(FilterType::Peaking, defaults[i % defaults.len()], 0.0, 1.0))
        .collect()
}

pub fn save_profile_bands(path: &Path, key: &str, bands: &[FilterBand]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut raw: BTreeMap<String, Vec<FilterBand>> = fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    raw.insert(
        key.to_string(),
        bands.iter().map(FilterBand::clamped).collect(),
    );
    let json = serde_json::to_string_pretty(&raw).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
