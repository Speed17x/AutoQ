//! Configuration, presets, and cache path management with seamless migration
//! from legacy `~/.config/autoq`, `~/.config/chu2-dsp-widget`, and Windows `AutoQ`
//! into `trivium`.

use crate::biquad::{flat_bands, FilterBand, FilterType};
use serde::{Deserialize, Serialize};
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

const MIGRATION_MARKER: &str = ".migrated_from_autoq";

fn migrate_config_files(src_dir: &Path, dst_dir: &Path) -> bool {
    if !src_dir.is_dir() {
        return false;
    }
    let _ = fs::create_dir_all(dst_dir);
    for fname in [
        "presets.json",
        "bt_profiles.json",
        "daemon_profiles.json",
        "ui_settings.json",
        "win_eq_state.json",
        "pw-moondrop-eq.conf",
        "pw-trivium-eq.conf",
    ] {
        let src = src_dir.join(fname);
        let dst = dst_dir.join(fname);
        if src.is_file() && !dst.exists() {
            let _ = fs::copy(&src, &dst);
        }
    }
    let _ = fs::write(dst_dir.join(MIGRATION_MARKER), b"1\n");
    true
}

pub fn config_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"))
    } else if cfg!(target_os = "macos") {
        env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join("Library").join("Application Support"))
    } else {
        env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"))
    };

    let primary = if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        base.join("Trivium")
    } else {
        base.join("trivium")
    };

    // One-time transparent migration from legacy AutoQ / chu2-dsp-widget directories
    if !primary.join(MIGRATION_MARKER).exists() {
        if cfg!(target_os = "windows") {
            let legacy_autoq = base.join("AutoQ");
            if legacy_autoq.is_dir() {
                migrate_config_files(&legacy_autoq, &primary);
            }
        } else {
            let mut migrated = false;
            for legacy_name in ["autoq", "chu2-dsp-widget"] {
                let legacy = base.join(legacy_name);
                if legacy.is_dir() {
                    migrated = migrate_config_files(&legacy, &primary);
                    break;
                }
            }
            if cfg!(target_os = "macos") && !migrated {
                let xdg_legacy = home_dir().join(".config").join("autoq");
                if xdg_legacy.is_dir() {
                    migrate_config_files(&xdg_legacy, &primary);
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
    } else if cfg!(target_os = "macos") {
        env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join("Library").join("Caches"))
    } else {
        env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".cache"))
    };

    let primary = if cfg!(target_os = "windows") {
        base.join("Trivium").join("cache")
    } else if cfg!(target_os = "macos") {
        base.join("Trivium")
    } else {
        base.join("trivium")
    };

    // Reuse existing AutoEQ catalog cache from AutoQ or chu2-dsp-widget if present
    if !primary.join("catalog_v2.json").exists() {
        if cfg!(target_os = "windows") {
            let legacy = base.join("AutoQ").join("cache");
            if legacy.join("catalog_v2.json").is_file() {
                return legacy;
            }
        } else {
            for legacy_name in ["autoq", "chu2-dsp-widget"] {
                let legacy = base.join(legacy_name);
                if legacy.join("catalog_v2.json").is_file() {
                    return legacy;
                }
            }
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

pub fn daemon_profiles_file() -> PathBuf {
    config_dir().join("daemon_profiles.json")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiThemeId {
    #[default]
    PureHyper,
    PureLight,
    NothingOs,
}

impl UiThemeId {
    pub fn all() -> &'static [UiThemeId] {
        &[UiThemeId::PureHyper, UiThemeId::PureLight, UiThemeId::NothingOs]
    }

    pub fn label(self) -> &'static str {
        match self {
            UiThemeId::PureHyper => "Default",
            UiThemeId::PureLight => "Default claro",
            UiThemeId::NothingOs => "NothingOS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiAccentId {
    #[default]
    Auto,
    Blue,
    Red,
    Emerald,
    Purple,
    Orange,
    Cyan,
    Pink,
}

impl UiAccentId {
    pub fn all() -> &'static [UiAccentId] {
        &[
            UiAccentId::Auto,
            UiAccentId::Blue,
            UiAccentId::Red,
            UiAccentId::Emerald,
            UiAccentId::Purple,
            UiAccentId::Orange,
            UiAccentId::Cyan,
            UiAccentId::Pink,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            UiAccentId::Auto => "Tema",
            UiAccentId::Blue => "Azul",
            UiAccentId::Red => "Rojo",
            UiAccentId::Emerald => "Verde",
            UiAccentId::Purple => "Violeta",
            UiAccentId::Orange => "Naranja",
            UiAccentId::Cyan => "Cian",
            UiAccentId::Pink => "Rosa",
        }
    }

    pub const fn rgb(self, is_dark: bool) -> Option<(u8, u8, u8)> {
        match self {
            UiAccentId::Auto => None,
            UiAccentId::Blue => Some(if is_dark { (0x34, 0x82, 0xFF) } else { (0x25, 0x63, 0xEB) }),
            UiAccentId::Red => Some(if is_dark { (0xD7, 0x19, 0x21) } else { (0xDC, 0x26, 0x26) }),
            UiAccentId::Emerald => Some(if is_dark { (0x10, 0xB9, 0x81) } else { (0x05, 0x96, 0x69) }),
            UiAccentId::Purple => Some(if is_dark { (0x8B, 0x5C, 0xF6) } else { (0x7C, 0x3A, 0xED) }),
            UiAccentId::Orange => Some(if is_dark { (0xF9, 0x73, 0x16) } else { (0xEA, 0x58, 0x0C) }),
            UiAccentId::Cyan => Some(if is_dark { (0x0E, 0xA5, 0xE9) } else { (0x02, 0x84, 0xC7) }),
            UiAccentId::Pink => Some(if is_dark { (0xEC, 0x48, 0x99) } else { (0xDB, 0x27, 0x77) }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    #[default]
    Auto,
    En,
    Es,
}

impl UiLanguage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::En => "ENGLISH",
            Self::Es => "ESPAÑOL",
        }
    }

    pub fn all() -> &'static [UiLanguage] {
        &[Self::Auto, Self::En, Self::Es]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiSettings {
    #[serde(default)]
    pub theme: UiThemeId,
    #[serde(default)]
    pub accent: UiAccentId,
    #[serde(default)]
    pub language: UiLanguage,
}

impl UiSettings {
    pub fn resolved_palette_json(&self) -> serde_json::Value {
        let (is_dark, bg, panel, border, default_accent, text_primary, text_muted, use_dot_grid) =
            match self.theme {
                UiThemeId::PureHyper => (
                    true,
                    "#0E1116",
                    "#171B22",
                    "#28303D",
                    "#3482FF",
                    "#F2F5FA",
                    "#949FB2",
                    false,
                ),
                UiThemeId::PureLight => (
                    false,
                    "#F4F6FA",
                    "#FFFFFF",
                    "#D5DCE6",
                    "#2563EB",
                    "#111827",
                    "#525E71",
                    false,
                ),
                UiThemeId::NothingOs => (
                    true,
                    "#0B0C0E",
                    "#131518",
                    "#272A32",
                    "#D71921",
                    "#F5F5F5",
                    "#8E939E",
                    true,
                ),
            };
        let accent_hex = match self.accent.rgb(is_dark) {
            Some((r, g, b)) => format!("#{r:02X}{g:02X}{b:02X}"),
            None => default_accent.to_string(),
        };
        serde_json::json!({
            "theme": self.theme,
            "accent": self.accent,
            "is_dark": is_dark,
            "bg": bg,
            "panel": panel,
            "border": border,
            "accent_hex": accent_hex,
            "text_primary": text_primary,
            "text_muted": text_muted,
            "use_dot_grid": use_dot_grid,
        })
    }
}

pub fn ui_settings_file() -> PathBuf {
    config_dir().join("ui_settings.json")
}

pub fn load_ui_settings() -> UiSettings {
    let path = ui_settings_file();
    if let Ok(txt) = fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str::<UiSettings>(&txt) {
            return s;
        }
    }
    UiSettings::default()
}

pub fn save_ui_settings(settings: &UiSettings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(ui_settings_file(), json).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoProfileRule {
    pub match_pattern: String,
    #[serde(default)]
    pub preset_name: Option<String>,
    #[serde(default)]
    pub bands: Vec<FilterBand>,
    #[serde(default = "default_true")]
    pub enable_eq: bool,
    #[serde(default)]
    pub commit_flash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_true")]
    pub auto_apply_saved_pipewire: bool,
    #[serde(default)]
    pub rules: Vec<AutoProfileRule>,
}

fn default_true() -> bool {
    true
}

fn default_poll_interval() -> u64 {
    2000
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_ms: default_poll_interval(),
            auto_apply_saved_pipewire: true,
            rules: Vec::new(),
        }
    }
}

pub fn load_daemon_config() -> DaemonConfig {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let path = daemon_profiles_file();
    if let Ok(txt) = fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<DaemonConfig>(&txt) {
            return cfg;
        }
    }
    DaemonConfig::default()
}

pub fn save_daemon_config(cfg: &DaemonConfig) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = daemon_profiles_file();
    let mut normalized = cfg.clone();
    for r in &mut normalized.rules {
        r.bands = r.bands.iter().map(FilterBand::clamped).collect();
    }
    let json = serde_json::to_string_pretty(&normalized).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn match_rule_for_device<'a>(
    cfg: &'a DaemonConfig,
    device_id: &str,
    display_name: &str,
    vid_pid: &str,
) -> Option<&'a AutoProfileRule> {
    let id_low = device_id.to_ascii_lowercase();
    let name_low = display_name.to_ascii_lowercase();
    let vp_low = vid_pid.to_ascii_lowercase();

    cfg.rules.iter().find(|r| {
        let pat = r.match_pattern.trim().to_ascii_lowercase();
        !pat.is_empty()
            && (id_low == pat
                || id_low.contains(&pat)
                || name_low.contains(&pat)
                || (!vp_low.is_empty() && vp_low.contains(&pat)))
    })
}

pub fn resolve_rule_bands(
    rule: &AutoProfileRule,
    presets: &BTreeMap<String, Vec<FilterBand>>,
) -> Vec<FilterBand> {
    if !rule.bands.is_empty() {
        return rule.bands.iter().map(FilterBand::clamped).collect();
    }
    if let Some(pname) = &rule.preset_name {
        if let Some(bands) = presets.get(pname) {
            return bands.iter().map(FilterBand::clamped).collect();
        }
    }
    Vec::new()
}

pub fn upsert_device_rule(
    cfg: &mut DaemonConfig,
    match_pattern: String,
    preset_name: Option<String>,
    bands: Vec<FilterBand>,
    enable_eq: bool,
    commit_flash: bool,
) {
    let pat_low = match_pattern.trim().to_ascii_lowercase();
    let clean_bands: Vec<FilterBand> = bands.iter().map(FilterBand::clamped).collect();
    if let Some(existing) = cfg
        .rules
        .iter_mut()
        .find(|r| r.match_pattern.trim().to_ascii_lowercase() == pat_low)
    {
        existing.preset_name = preset_name;
        existing.bands = clean_bands;
        existing.enable_eq = enable_eq;
        existing.commit_flash = commit_flash;
    } else {
        cfg.rules.push(AutoProfileRule {
            match_pattern: match_pattern.trim().to_string(),
            preset_name,
            bands: clean_bands,
            enable_eq,
            commit_flash,
        });
    }
}

pub fn remove_device_rule(cfg: &mut DaemonConfig, match_pattern: &str) -> bool {
    let pat_low = match_pattern.trim().to_ascii_lowercase();
    let before = cfg.rules.len();
    cfg.rules
        .retain(|r| r.match_pattern.trim().to_ascii_lowercase() != pat_low);
    cfg.rules.len() != before
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_rule_matching_and_resolution() {
        let mut cfg = DaemonConfig::default();
        upsert_device_rule(
            &mut cfg,
            "Moondrop Chu 2 DSP".to_string(),
            Some("Harman Warm".to_string()),
            vec![FilterBand::new(FilterType::LowShelf, 105.0, 3.0, 0.707)],
            true,
            false,
        );
        upsert_device_rule(
            &mut cfg,
            "152a:88fa".to_string(),
            Some("Vocal / Ref".to_string()),
            Vec::new(),
            true,
            false,
        );

        let r1 = match_rule_for_device(&cfg, "usb:/dev/hidraw0", "Moondrop Chu 2 DSP", "31b2:0113");
        assert!(r1.is_some());
        let presets = default_presets();
        let b1 = resolve_rule_bands(r1.unwrap(), &presets);
        assert_eq!(b1.len(), 1);
        assert!((b1[0].gain - 3.0).abs() < 1e-6);

        let r2 = match_rule_for_device(&cfg, "usb:/dev/ttyACM0", "JDS Labs Element IV", "152a:88fa");
        assert!(r2.is_some());
        let b2 = resolve_rule_bands(r2.unwrap(), &presets);
        assert!(!b2.is_empty());

        assert!(remove_device_rule(&mut cfg, "152a:88fa"));
        assert!(match_rule_for_device(&cfg, "usb:/dev/ttyACM0", "JDS Labs Element IV", "152a:88fa").is_none());
    }

    #[test]
    fn ui_theme_defaults_to_pure_hyper_and_serializes() {
        let def = UiSettings::default();
        assert_eq!(def.theme, UiThemeId::PureHyper);
        assert_eq!(def.accent, UiAccentId::Auto);
        assert_eq!(def.language, UiLanguage::Auto);
        assert_eq!(UiThemeId::PureHyper.label(), "Default");
        assert_eq!(UiThemeId::PureLight.label(), "Default claro");
        assert_eq!(UiThemeId::NothingOs.label(), "NothingOS");

        let raw = serde_json::to_string(&UiSettings {
            theme: UiThemeId::NothingOs,
            accent: UiAccentId::Emerald,
            language: UiLanguage::Es,
        })
        .unwrap();
        assert!(raw.contains("nothing_os"));
        assert!(raw.contains("emerald"));
        assert!(raw.contains("\"language\":\"es\""));
        let decoded: UiSettings = serde_json::from_str(&raw).unwrap();
        assert_eq!(decoded.theme, UiThemeId::NothingOs);
        assert_eq!(decoded.accent, UiAccentId::Emerald);
        assert_eq!(decoded.language, UiLanguage::Es);

        let legacy: UiSettings = serde_json::from_str(r#"{"theme":"nothing_os"}"#).unwrap();
        assert_eq!(legacy.theme, UiThemeId::NothingOs);
        assert_eq!(legacy.accent, UiAccentId::Auto);
        assert_eq!(legacy.language, UiLanguage::Auto);
    }

    #[test]
    fn migrates_legacy_autoq_config_without_overwriting_or_copying_secrets() {
        let root = env::temp_dir().join(format!("trivium_mig_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let src = root.join("autoq");
        let dst = root.join("trivium");
        fs::create_dir_all(src.join("signing")).unwrap();
        fs::create_dir_all(&dst).unwrap();

        fs::write(src.join("presets.json"), r#"{"Custom":[]}"#).unwrap();
        fs::write(src.join("daemon_profiles.json"), r#"{"rules":[]}"#).unwrap();
        fs::write(src.join("ui_settings.json"), r#"{"theme":"nothing_os"}"#).unwrap();
        fs::write(src.join("signing/secret.key"), "DO-NOT-COPY").unwrap();

        // Pre-existing destination ui_settings.json must be preserved
        fs::write(dst.join("ui_settings.json"), r#"{"theme":"pure_hyper"}"#).unwrap();

        assert!(migrate_config_files(&src, &dst));
        assert_eq!(
            fs::read_to_string(dst.join("presets.json")).unwrap(),
            r#"{"Custom":[]}"#
        );
        assert_eq!(
            fs::read_to_string(dst.join("daemon_profiles.json")).unwrap(),
            r#"{"rules":[]}"#
        );
        assert_eq!(
            fs::read_to_string(dst.join("ui_settings.json")).unwrap(),
            r#"{"theme":"pure_hyper"}"#
        );
        assert!(!dst.join("signing/secret.key").exists());
        assert!(dst.join(MIGRATION_MARKER).is_file());

        let _ = fs::remove_dir_all(&root);
    }
}
