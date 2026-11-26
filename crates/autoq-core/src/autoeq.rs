//! Multi-source AutoEq + 7 Squiglink database client, SHA-1 disk cache,
//! FR curve reconstructor, and PEQ matcher.

use crate::biquad::{
    band_response_db, combined_response_db, fit_peq_bands, geomspace, interp_log_freq,
    make_weights, standard_grid, weighted_rmse, FilterBand, FilterType, SAMPLE_RATE_HZ,
};
use crate::config::cache_dir;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

pub const INDEX_URL: &str =
    "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/INDEX.md";
pub const RESULTS_BASE_URL: &str =
    "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results";
pub const TARGETS_BASE_URL: &str =
    "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/targets";

#[derive(Debug, Clone)]
pub struct SquigSource {
    pub id: &'static str,
    pub label: &'static str,
    pub base: &'static str,
    pub ff: &'static str,
    pub rig: &'static str,
}

pub const SQUIG_SOURCES: &[SquigSource] = &[
    SquigSource {
        id: "superreview",
        label: "Super* Review (711)",
        base: "https://squig.link/data/",
        ff: "in-ear",
        rig: "711",
    },
    SquigSource {
        id: "regancipher",
        label: "ReganCipher (TWS/711)",
        base: "https://regancipher.squig.link/data/",
        ff: "in-ear",
        rig: "711",
    },
    SquigSource {
        id: "aftersound",
        label: "Aftersound (711)",
        base: "https://aftersound.squig.link/data/",
        ff: "in-ear",
        rig: "711",
    },
    SquigSource {
        id: "hbb",
        label: "HBB (711)",
        base: "https://hbb.squig.link/data/",
        ff: "in-ear",
        rig: "711",
    },
    SquigSource {
        id: "paulwasabii",
        label: "Paul Wasabii (711)",
        base: "https://pw.squig.link/data/",
        ff: "in-ear",
        rig: "711",
    },
    SquigSource {
        id: "jaytiss",
        label: "Jaytiss (711)",
        base: "https://jaytiss.squig.link/data/",
        ff: "in-ear",
        rig: "711",
    },
    SquigSource {
        id: "gadgetrytech",
        label: "GadgetryTech (Over-Ear)",
        base: "https://gadgetrytech.squig.link/headsets/data/",
        ff: "over-ear",
        rig: "over-ear",
    },
];

pub const CURATED_TARGETS: &[(&str, &str)] = &[
    ("AutoEq In-Ear (Predeterminado)", ""),
    ("Harman In-Ear 2019", "Harman in-ear 2019.csv"),
    (
        "Harman In-Ear 2019 (sin boost bajos)",
        "Harman in-ear 2019 without bass.csv",
    ),
    ("Harman In-Ear 2017-2", "Harman in-ear 2017-2.csv"),
    ("Harman Over-Ear 2018", "Harman over-ear 2018.csv"),
    ("JM-1 con filtros Harman", "JM-1 with Harman filters.csv"),
    (
        "Diffuse Field 5128 (-1 dB/oct)",
        "Diffuse field 5128 -1dB per octave.csv",
    ),
    ("Oratory1990 In-Ear", "oratory1990 in-ear.csv"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoEqEntry {
    pub name: String,
    pub rel_path: String,
    pub source_rig: String,
    pub source_kind: String, // "autoeq" | "squiglink"
    pub squig_base_url: String,
    pub squig_file_stem: String,
    pub form_factor_override: String,
    pub rig_tag_override: String,
}

impl AutoEqEntry {
    pub fn unique_key(&self) -> String {
        let prefix = if self.source_kind == "squiglink" {
            "sq"
        } else {
            "ae"
        };
        format!("{prefix}:{}", self.rel_path)
    }

    pub fn form_factor(&self) -> String {
        if !self.form_factor_override.is_empty() {
            return self.form_factor_override.clone();
        }
        let low = percent_decode(&self.rel_path).to_ascii_lowercase();
        if low.contains("in-ear") {
            "in-ear".to_string()
        } else if low.contains("over-ear") {
            "over-ear".to_string()
        } else if low.contains("earbud") {
            "earbud".to_string()
        } else {
            "in-ear".to_string()
        }
    }

    pub fn rig_tag(&self) -> String {
        if !self.rig_tag_override.is_empty() {
            return self.rig_tag_override.clone();
        }
        let low = self.source_rig.to_ascii_lowercase();
        if low.contains("5128") {
            "5128".to_string()
        } else if low.contains("4620") {
            "4620".to_string()
        } else if low.contains("hms") {
            "HMS".to_string()
        } else if [
            "711",
            "super* review",
            "super review",
            "oratory1990",
            "tonedeafmonk",
            "aftersound",
            "hbb",
            "paul wasabii",
            "jaytiss",
            "regancipher",
        ]
        .iter()
        .any(|k| low.contains(k))
        {
            "711".to_string()
        } else {
            self.source_rig.clone()
        }
    }

    pub fn folder_segment(&self) -> &str {
        self.rel_path.rsplit('/').next().unwrap_or(&self.rel_path)
    }

    pub fn graphic_eq_url(&self) -> String {
        format!(
            "{RESULTS_BASE_URL}/{}/{}%20GraphicEQ.txt",
            self.rel_path,
            self.folder_segment()
        )
    }

    pub fn parametric_eq_url(&self) -> String {
        format!(
            "{RESULTS_BASE_URL}/{}/{}%20ParametricEQ.txt",
            self.rel_path,
            self.folder_segment()
        )
    }
}

pub fn normalize_search_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = true;
    for ch in raw.chars() {
        let mapped = match ch {
            ':' | '-' | '_' => ' ',
            c => c,
        };
        if mapped.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            for lc in mapped.to_lowercase() {
                out.push(lc);
            }
            prev_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn sha1_hex(input: &str, len: usize) -> String {
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    let res = hasher.finalize();
    let hex: String = res.iter().map(|b| format!("{b:02x}")).collect();
    hex[..len.min(hex.len())].to_string()
}

fn percent_encode_all(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(s) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(s, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn ensure_cache_dirs(base: &Path) {
    let _ = fs::create_dir_all(base.join("profiles"));
    let _ = fs::create_dir_all(base.join("targets"));
}

fn fetch_text(
    url: &str,
    cache_file: &Path,
    force_refresh: bool,
    timeout_secs: u64,
) -> Result<String, String> {
    if let Some(parent) = cache_file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if !force_refresh {
        if let Ok(meta) = fs::metadata(cache_file) {
            if meta.len() > 0 {
                if let Ok(txt) = fs::read_to_string(cache_file) {
                    return Ok(txt);
                }
            }
        }
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("AutoQ-Rust/1.1")
        .build();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("Error descargando {url}: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| format!("Error leyendo respuesta de {url}: {e}"))?;
    let _ = fs::write(cache_file, &text);
    Ok(text)
}

pub fn parse_autoeq_index_line(line: &str) -> Option<AutoEqEntry> {
    // Matches: `- [Name](./rel_path) by Source Rig`
    let rest = line.trim().strip_prefix("- [")?;
    let mid = rest.find("](./")?;
    let name = &rest[..mid];
    let after_mid = &rest[mid + 4..];
    let end_paren = after_mid.rfind(") by ")?;
    let rel_path = &after_mid[..end_paren];
    let source_rig = &after_mid[end_paren + 5..];
    if name.is_empty() || rel_path.is_empty() || source_rig.is_empty() {
        return None;
    }
    Some(AutoEqEntry {
        name: name.to_string(),
        rel_path: rel_path.to_string(),
        source_rig: source_rig.to_string(),
        source_kind: "autoeq".to_string(),
        squig_base_url: String::new(),
        squig_file_stem: String::new(),
        form_factor_override: String::new(),
        rig_tag_override: String::new(),
    })
}

pub fn parse_squig_phone_book(raw_json: &str, src: &SquigSource) -> Vec<AutoEqEntry> {
    let Ok(serde_json::Value::Array(brands)) = serde_json::from_str::<serde_json::Value>(raw_json)
    else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    for brand in brands {
        let Some(bobj) = brand.as_object() else {
            continue;
        };
        let bname = bobj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if bname.starts_with('_') {
            continue;
        }
        let Some(phones) = bobj.get("phones").and_then(|v| v.as_array()) else {
            continue;
        };
        for ph in phones {
            let Some(pobj) = ph.as_object() else {
                continue;
            };
            let pname = pobj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if pname.is_empty() || pname.starts_with('_') {
                continue;
            }
            let full_base = if !bname.is_empty()
                && !pname
                    .to_ascii_lowercase()
                    .starts_with(&bname.to_ascii_lowercase())
            {
                format!("{bname} {pname}").trim().to_string()
            } else {
                pname.to_string()
            };

            let files: Vec<String> = match pobj.get("file") {
                Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
                    vec![s.trim().to_string()]
                }
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .collect(),
                _ => Vec::new(),
            };
            if files.is_empty() {
                continue;
            }

            let suffixes: Vec<String> = match pobj.get("suffix") {
                Some(serde_json::Value::String(s)) => vec![s.trim().to_string()],
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .map(|x| x.as_str().unwrap_or("").trim().to_string())
                    .collect(),
                _ => Vec::new(),
            };

            for (idx, stem) in files.iter().enumerate() {
                let suf = suffixes.get(idx).map(|s| s.as_str()).unwrap_or("");
                let variant = if !suf.is_empty() {
                    format!("{full_base} {suf}").trim().to_string()
                } else if files.len() > 1 && idx > 0 {
                    format!("{full_base} (#{})", idx + 1)
                } else {
                    full_base.clone()
                };
                entries.push(AutoEqEntry {
                    name: variant,
                    rel_path: format!("{}/{stem}", src.id),
                    source_rig: src.label.to_string(),
                    source_kind: "squiglink".to_string(),
                    squig_base_url: src.base.to_string(),
                    squig_file_stem: stem.clone(),
                    form_factor_override: src.ff.to_string(),
                    rig_tag_override: src.rig.to_string(),
                });
            }
        }
    }
    entries
}

pub fn load_catalog(force_refresh: bool) -> (Vec<AutoEqEntry>, usize, usize) {
    let cdir = cache_dir();
    ensure_cache_dirs(&cdir);
    let total_sources = 1 + SQUIG_SOURCES.len();
    let json_cache = cdir.join("catalog_v2.json");

    if !force_refresh && json_cache.is_file() {
        if let Ok(txt) = fs::read_to_string(&json_cache) {
            if let Ok(raw) = serde_json::from_str::<Vec<[String; 8]>>(&txt) {
                let entries: Vec<AutoEqEntry> = raw
                    .into_iter()
                    .map(|x| AutoEqEntry {
                        name: x[0].clone(),
                        rel_path: x[1].clone(),
                        source_rig: x[2].clone(),
                        source_kind: x[3].clone(),
                        squig_base_url: x[4].clone(),
                        squig_file_stem: x[5].clone(),
                        form_factor_override: x[6].clone(),
                        rig_tag_override: x[7].clone(),
                    })
                    .collect();
                if !entries.is_empty() {
                    let active_count = entries
                        .iter()
                        .map(|e| (e.source_kind.as_str(), e.squig_base_url.as_str()))
                        .collect::<HashSet<_>>()
                        .len()
                        .clamp(1, total_sources);
                    return (entries, active_count, total_sources);
                }
            }
        }
    }

    let cdir_ref = &cdir;
    let (squig_batches, ae_batch) = thread::scope(|s| {
        let ae_handle = s.spawn(move || -> Vec<AutoEqEntry> {
            let Ok(text) = fetch_text(INDEX_URL, &cdir_ref.join("index.md"), force_refresh, 16)
            else {
                return Vec::new();
            };
            text.lines().filter_map(parse_autoeq_index_line).collect()
        });

        let sq_handles: Vec<_> = SQUIG_SOURCES
            .iter()
            .map(|src| {
                s.spawn(move || -> Vec<AutoEqEntry> {
                    let url = format!("{}phone_book.json", src.base);
                    let cache_f = cdir_ref.join(format!("squig_{}.json", src.id));
                    let Ok(raw_txt) = fetch_text(&url, &cache_f, force_refresh, 10) else {
                        return Vec::new();
                    };
                    parse_squig_phone_book(&raw_txt, src)
                })
            })
            .collect();

        let sq_res: Vec<Vec<AutoEqEntry>> = sq_handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect();
        let ae_res = ae_handle.join().unwrap_or_default();
        (sq_res, ae_res)
    });

    let mut combined = Vec::new();
    let mut ok_sources = 0usize;
    for batch in squig_batches {
        if !batch.is_empty() {
            ok_sources += 1;
            combined.extend(batch);
        }
    }
    if !ae_batch.is_empty() {
        ok_sources += 1;
        combined.extend(ae_batch);
    }

    if !combined.is_empty() {
        let compact: Vec<[&str; 8]> = combined
            .iter()
            .map(|e| {
                [
                    e.name.as_str(),
                    e.rel_path.as_str(),
                    e.source_rig.as_str(),
                    e.source_kind.as_str(),
                    e.squig_base_url.as_str(),
                    e.squig_file_stem.as_str(),
                    e.form_factor_override.as_str(),
                    e.rig_tag_override.as_str(),
                ]
            })
            .collect();
        if let Ok(serialized) = serde_json::to_string(&compact) {
            let _ = fs::write(&json_cache, serialized);
        }
    }

    (combined, ok_sources, total_sources)
}

pub fn find_best_entry_any_brand<'a>(
    entries: &'a [AutoEqEntry],
    hint: &str,
) -> Option<&'a AutoEqEntry> {
    if entries.is_empty() || hint.trim().is_empty() {
        return None;
    }
    let hint_norm = normalize_search_text(hint);
    let tokens: Vec<&str> = hint_norm
        .split_whitespace()
        .filter(|t| t.len() >= 2)
        .collect();
    if tokens.is_empty() {
        return None;
    }

    let mut best: Option<&'a AutoEqEntry> = None;
    let mut best_score = 0usize;

    for e in entries {
        let low = normalize_search_text(&e.name);
        let matched = tokens.iter().filter(|&&tok| low.contains(tok)).count();
        if matched == 0 {
            continue;
        }
        let mut score = matched * 14;
        if matched == tokens.len() {
            score += 18;
        }
        if low == hint_norm {
            score += 60;
        } else if low.starts_with(&hint_norm) {
            score += 26;
        } else if low.contains(&hint_norm) {
            score += 16;
        }
        let rig = e.source_rig.to_ascii_lowercase();
        if rig.contains("super* review") {
            score += 7;
        } else if rig.contains("oratory1990") || rig.contains("crinacle on 711") {
            score += 6;
        } else if rig.contains("regancipher") {
            score += 5;
        } else if rig.contains("aftersound") {
            score += 4;
        }
        if score > best_score {
            best_score = score;
            best = Some(e);
        }
    }
    best
}

pub fn parse_squig_fr_txt(text: &str) -> Result<(Vec<f64>, Vec<f64>), String> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || matches!(line.as_bytes()[0], b'*' | b'#' | b';') {
            continue;
        }
        let mut iter = line.split(|c: char| c.is_whitespace() || c == ',' || c == ';');
        let Some(p0) = iter.find(|s| !s.is_empty()) else {
            continue;
        };
        let Some(p1) = iter.find(|s| !s.is_empty()) else {
            continue;
        };
        if let (Ok(f), Ok(db_v)) = (p0.parse::<f64>(), p1.parse::<f64>()) {
            if f.is_finite() && db_v.is_finite() && (10.0..=25_000.0).contains(&f) {
                pts.push((f, db_v));
            }
        }
    }
    if pts.len() < 10 {
        return Err("Archivo FR Squiglink inválido o vacío.".to_string());
    }
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut out_f = Vec::with_capacity(pts.len());
    let mut out_db = Vec::with_capacity(pts.len());
    let mut i = 0;
    while i < pts.len() {
        let f0 = pts[i].0;
        let mut acc = pts[i].1;
        let mut cnt = 1usize;
        while i + 1 < pts.len() && (pts[i + 1].0 - f0).abs() < 1e-6 {
            i += 1;
            acc += pts[i].1;
            cnt += 1;
        }
        out_f.push(f0);
        out_db.push(acc / (cnt as f64));
        i += 1;
    }
    Ok((out_f, out_db))
}

pub fn parse_graphic_eq(text: &str) -> Result<(Vec<f64>, Vec<f64>), String> {
    let trimmed = text.trim();
    let body = if trimmed.len() >= 10 && trimmed[..10].eq_ignore_ascii_case("graphiceq:") {
        &trimmed[10..]
    } else {
        trimmed
    };
    let mut freqs = Vec::new();
    let mut gains = Vec::new();
    for pair in body.split(';') {
        let mut parts = pair.split_whitespace();
        if let (Some(fs), Some(gs)) = (parts.next(), parts.next()) {
            if let (Ok(f), Ok(g)) = (fs.parse::<f64>(), gs.parse::<f64>()) {
                freqs.push(f);
                gains.push(g);
            }
        }
    }
    if freqs.len() < 5 {
        return Err("Formato GraphicEQ inválido o incompleto.".to_string());
    }
    Ok((freqs, gains))
}

pub fn parse_parametric_eq(text: &str) -> Vec<FilterBand> {
    // Matches lines like: `Filter 1: ON PK Fc 105 Hz Gain 3.2 dB Q 0.707`
    let mut bands = Vec::new();
    for raw in text.lines() {
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 12 {
            continue;
        }
        if !tokens[0].eq_ignore_ascii_case("filter") || !tokens[2].eq_ignore_ascii_case("on") {
            continue;
        }
        let ftype_tok = tokens[3].to_ascii_uppercase();
        if !matches!(ftype_tok.as_str(), "PK" | "LSC" | "LS" | "HSC" | "HS") {
            continue;
        }
        let ftype = FilterType::from_loose_str(&ftype_tok);

        let mut fc = None;
        let mut gain = None;
        let mut q = None;
        let mut i = 4;
        while i + 1 < tokens.len() {
            if tokens[i].eq_ignore_ascii_case("fc") {
                fc = tokens[i + 1].parse::<f64>().ok();
            } else if tokens[i].eq_ignore_ascii_case("gain") {
                gain = tokens[i + 1].parse::<f64>().ok();
            } else if tokens[i].eq_ignore_ascii_case("q") {
                q = tokens[i + 1].parse::<f64>().ok();
            }
            i += 1;
        }
        if let (Some(f), Some(g), Some(qv)) = (fc, gain, q) {
            bands.push(FilterBand::new(ftype, f, g, qv).clamped());
        }
    }
    bands
}

pub fn export_parametric_eq(bands: &[FilterBand]) -> String {
    export_parametric_eq_with_preamp(bands, 0.0)
}

pub fn export_parametric_eq_with_preamp(bands: &[FilterBand], preamp_db: f64) -> String {
    let preamp = if preamp_db.is_finite() {
        preamp_db.clamp(-18.0, 6.0)
    } else {
        0.0
    };
    let mut lines = Vec::with_capacity(bands.len() + 1);
    lines.push(format!("Preamp: {preamp:.1} dB"));
    for (i, b) in bands.iter().enumerate() {
        let bc = b.clamped();
        lines.push(format!(
            "Filter {}: ON {} Fc {} Hz Gain {:.1} dB Q {:.3}",
            i + 1,
            bc.filter_type.apo_code(),
            bc.frequency.round() as i64,
            bc.gain,
            bc.q
        ));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn export_easyeffects_json(bands: &[FilterBand], preamp_db: f64) -> String {
    let clean: Vec<FilterBand> = bands.iter().map(FilterBand::clamped).collect();
    let preamp = if preamp_db.is_finite() {
        (preamp_db.clamp(-18.0, 6.0) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let mut ch_bands = serde_json::Map::new();
    for (i, b) in clean.iter().enumerate() {
        let ftype = match b.filter_type {
            FilterType::Peaking => "Bell",
            FilterType::LowShelf => "Lo-shelf",
            FilterType::HighShelf => "Hi-shelf",
        };
        ch_bands.insert(
            format!("band{i}"),
            serde_json::json!({
                "frequency": b.frequency.round(),
                "gain": b.gain,
                "mode": "RLC (BT)",
                "mute": false,
                "q": b.q,
                "slope": "x1",
                "solo": false,
                "type": ftype,
                "width": 4.0,
            }),
        );
    }
    let eq_obj = serde_json::json!({
        "output": {
            "blocklist": [],
            "plugins_order": ["equalizer#0"],
            "equalizer#0": {
                "balance": 0.0,
                "bypass": false,
                "input-gain": preamp,
                "left": serde_json::Value::Object(ch_bands.clone()),
                "mode": "IIR",
                "num-bands": clean.len(),
                "output-gain": 0.0,
                "pitch-left": 0.0,
                "pitch-right": 0.0,
                "right": serde_json::Value::Object(ch_bands),
                "split-channels": false,
            }
        }
    });
    serde_json::to_string_pretty(&eq_obj).unwrap_or_default()
}

pub fn export_camilladsp_yaml(bands: &[FilterBand], preamp_db: f64) -> String {
    let clean: Vec<FilterBand> = bands.iter().map(FilterBand::clamped).collect();
    let preamp = if preamp_db.is_finite() {
        (preamp_db.clamp(-18.0, 6.0) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let mut out = String::from("# Generado por AutoQ — CamillaDSP v2 Filters\nfilters:\n");
    out.push_str(&format!(
        "  preamp:\n    type: Gain\n    parameters:\n      gain: {preamp:.2}\n      inverted: false\n      mute: false\n"
    ));
    let mut names = vec!["preamp".to_string()];
    for (i, b) in clean.iter().enumerate() {
        let name = format!("band_{:02}", i + 1);
        let bq_type = match b.filter_type {
            FilterType::Peaking => "Peaking",
            FilterType::LowShelf => "Lowshelf",
            FilterType::HighShelf => "Highshelf",
        };
        out.push_str(&format!(
            "  {name}:\n    type: Biquad\n    parameters:\n      type: {bq_type}\n      freq: {}\n      gain: {:.2}\n      q: {:.3}\n",
            b.frequency.round() as i64,
            b.gain,
            b.q
        ));
        names.push(name);
    }
    out.push_str("pipeline:\n  - type: Filter\n    channels: [0, 1]\n    names:\n");
    for n in names {
        out.push_str(&format!("      - {n}\n"));
    }
    out
}

pub fn export_wavelet_graphiceq(bands: &[FilterBand]) -> String {
    let freqs = crate::biquad::geomspace(20.0, 20_000.0, 127);
    let resp = crate::biquad::combined_response_db(bands, &freqs, crate::biquad::SAMPLE_RATE_HZ);
    let max_pos = resp
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(0.0_f64, f64::max);
    let pairs: Vec<String> = freqs
        .iter()
        .zip(resp.iter())
        .map(|(&f, &g)| {
            let norm = ((g - max_pos) * 10.0).round() / 10.0;
            format!("{} {:.1}", f.round() as i64, norm)
        })
        .collect();
    format!("GraphicEQ: {}\n", pairs.join("; "))
}

pub fn fetch_target_csv(filename: &str, force_refresh: bool) -> Result<(Vec<f64>, Vec<f64>), String> {
    let cdir = cache_dir();
    let encoded = percent_encode_all(filename);
    let url = format!("{TARGETS_BASE_URL}/{encoded}");
    let safe_name = format!("{}.csv", sha1_hex(filename, 12));
    let text = fetch_text(
        &url,
        &cdir.join("targets").join(safe_name),
        force_refresh,
        12,
    )?;

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut freqs = Vec::new();
    let mut vals = Vec::new();
    for result in rdr.records() {
        let Ok(rec) = result else {
            continue;
        };
        if rec.len() >= 2 {
            if let (Ok(f), Ok(v)) = (rec[0].trim().parse::<f64>(), rec[1].trim().parse::<f64>()) {
                freqs.push(f);
                vals.push(v);
            }
        }
    }
    if freqs.is_empty() {
        return Err(format!("Target CSV vacío o inválido: {filename}"));
    }
    Ok((freqs, vals))
}

fn mean_in_band(freqs: &[f64], curve_db: &[f64], f_lo: f64, f_hi: f64) -> Option<f64> {
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for (&f, &v) in freqs.iter().zip(curve_db.iter()) {
        if (f_lo..=f_hi).contains(&f) {
            sum += v;
            cnt += 1;
        }
    }
    if cnt > 0 {
        Some(sum / (cnt as f64))
    } else {
        None
    }
}

fn center_midband(freqs: &[f64], curve_db: &[f64]) -> Vec<f64> {
    let offset = mean_in_band(freqs, curve_db, 200.0, 3000.0).unwrap_or_else(|| {
        if curve_db.is_empty() {
            0.0
        } else {
            curve_db.iter().sum::<f64>() / (curve_db.len() as f64)
        }
    });
    curve_db.iter().map(|&v| v - offset).collect()
}

fn normalize_800hz(freqs: &[f64], curve_db: &[f64]) -> Vec<f64> {
    let offset = mean_in_band(freqs, curve_db, 400.0, 1200.0).unwrap_or(0.0);
    curve_db.iter().map(|&v| v - offset).collect()
}

fn default_autoeq_baseline(freqs: &[f64], form_factor: &str) -> Result<Vec<f64>, String> {
    if form_factor == "over-ear" {
        let (f_b, b_db) = fetch_target_csv("Harman over-ear 2018.csv", false)?;
        return Ok(interp_log_freq(freqs, &f_b, &b_db));
    }
    let (f_b, b_db) = fetch_target_csv("AutoEq in-ear.csv", false)?;
    let mut base = interp_log_freq(freqs, &f_b, &b_db);
    let bass_shelf = band_response_db(
        &FilterBand::new(FilterType::LowShelf, 105.0, 8.0, 0.7),
        freqs,
        SAMPLE_RATE_HZ,
    );
    for (b, &s) in base.iter_mut().zip(bass_shelf.iter()) {
        *b += s;
    }
    Ok(base)
}

fn try_fetch_squig_channel(
    base_url: &str,
    filename: &str,
    cache_path: &PathBuf,
) -> Option<(Vec<f64>, Vec<f64>)> {
    let url = format!("{base_url}{}", percent_encode_all(filename));
    let txt = fetch_text(&url, cache_path, false, 10).ok()?;
    parse_squig_fr_txt(&txt).ok()
}

pub fn reconstruct_fr_curve(entry: &AutoEqEntry) -> Result<(Vec<f64>, Vec<f64>), String> {
    let grid = standard_grid();
    let cdir = cache_dir();
    ensure_cache_dirs(&cdir);

    if entry.source_kind == "squiglink" {
        let digest = sha1_hex(&format!("{}|{}", entry.rel_path, entry.squig_base_url), 16);
        let l_name = format!("{} L.txt", entry.squig_file_stem);
        let r_name = format!("{} R.txt", entry.squig_file_stem);
        let l_cache = cdir.join("profiles").join(format!("{digest}_L.txt"));
        let r_cache = cdir.join("profiles").join(format!("{digest}_R.txt"));

        let (left, right) = thread::scope(|s| {
            let fl = s.spawn(|| try_fetch_squig_channel(&entry.squig_base_url, &l_name, &l_cache));
            let fr = s.spawn(|| try_fetch_squig_channel(&entry.squig_base_url, &r_name, &r_cache));
            (fl.join().ok().flatten(), fr.join().ok().flatten())
        });

        let merged = match (left, right) {
            (Some(l), Some(r)) => {
                let li = interp_log_freq(&grid, &l.0, &l.1);
                let ri = interp_log_freq(&grid, &r.0, &r.1);
                li.iter()
                    .zip(ri.iter())
                    .map(|(&a, &b)| 0.5 * (a + b))
                    .collect()
            }
            (Some(l), None) => interp_log_freq(&grid, &l.0, &l.1),
            (None, Some(r)) => interp_log_freq(&grid, &r.0, &r.1),
            (None, None) => {
                let m_name = format!("{}.txt", entry.squig_file_stem);
                let m_cache = cdir.join("profiles").join(format!("{digest}_M.txt"));
                let mono = try_fetch_squig_channel(&entry.squig_base_url, &m_name, &m_cache)
                    .ok_or_else(|| {
                        format!(
                            "No se pudo descargar la curva FR de {} ({}).",
                            entry.name, entry.source_rig
                        )
                    })?;
                interp_log_freq(&grid, &mono.0, &mono.1)
            }
        };
        let norm = normalize_800hz(&grid, &merged);
        return Ok((grid, norm));
    }

    let digest = sha1_hex(&entry.rel_path, 16);
    let cache_path = cdir.join("profiles").join(format!("{digest}_graphic.txt"));
    let text = fetch_text(&entry.graphic_eq_url(), &cache_path, false, 12)?;
    let (freqs, eq_db) = parse_graphic_eq(&text)?;
    let ref_curve = default_autoeq_baseline(&freqs, &entry.form_factor())?;
    let fr: Vec<f64> = ref_curve
        .iter()
        .zip(eq_db.iter())
        .map(|(&r, &e)| r - e)
        .collect();
    let interp = interp_log_freq(&grid, &freqs, &fr);
    let norm = normalize_800hz(&grid, &interp);
    Ok((grid, norm))
}

pub fn reconstruct_target_csv_fr(
    csv_name: Option<&str>,
    form_factor: &str,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let grid = standard_grid();
    match csv_name {
        None | Some("") => {
            let ref_curve = default_autoeq_baseline(&grid, form_factor)?;
            let norm = normalize_800hz(&grid, &ref_curve);
            Ok((grid, norm))
        }
        Some(name) => {
            let (freqs, ref_curve) = fetch_target_csv(name, false)?;
            let interp = interp_log_freq(&grid, &freqs, &ref_curve);
            let norm = normalize_800hz(&grid, &interp);
            Ok((grid, norm))
        }
    }
}

pub fn downsample_curve(freqs: &[f64], vals: &[f64], points: usize) -> Vec<[f64; 2]> {
    let grid = geomspace(20.0, 20_000.0, points);
    let interp = interp_log_freq(&grid, freqs, vals);
    grid.into_iter()
        .zip(interp)
        .map(|(f, v)| [((f * 10.0).round()) / 10.0, ((v * 100.0).round()) / 100.0])
        .collect()
}

pub struct MatchResult {
    pub freqs: Vec<f64>,
    pub delta_db: Vec<f64>,
    pub stock_fr: Vec<f64>,
    pub target_fr_aligned: Vec<f64>,
    pub bands: Vec<FilterBand>,
    pub rmse: f64,
    pub warning: Option<String>,
}

fn align_target_overlay(f_s: &[f64], fr_s: &[f64], f_t: &[f64], fr_t: &[f64], delta_db: &[f64]) -> Vec<f64> {
    let fr_t_on_s = interp_log_freq(f_s, f_t, fr_t);
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for i in 0..f_s.len() {
        if (200.0..=3000.0).contains(&f_s[i]) {
            sum += (fr_s[i] + delta_db[i]) - fr_t_on_s[i];
            cnt += 1;
        }
    }
    let shift = if cnt > 0 { sum / (cnt as f64) } else { 0.0 };
    fr_t_on_s.into_iter().map(|v| v + shift).collect()
}

pub fn compute_match_headphone_peq(
    source_entry: &AutoEqEntry,
    target_phone: &AutoEqEntry,
    max_freq_fit: f64,
    max_bands: usize,
    max_gain_db: f64,
    supports_shelves: bool,
) -> Result<MatchResult, String> {
    let (res_src, res_tgt) = thread::scope(|s| {
        let hs = s.spawn(|| reconstruct_fr_curve(source_entry));
        let ht = s.spawn(|| reconstruct_fr_curve(target_phone));
        (hs.join().unwrap(), ht.join().unwrap())
    });
    let (f_src, fr_src) = res_src?;
    let (f_tgt, fr_tgt) = res_tgt?;

    let fr_tgt_i = interp_log_freq(&f_src, &f_tgt, &fr_tgt);
    let raw_diff: Vec<f64> = fr_tgt_i
        .iter()
        .zip(fr_src.iter())
        .map(|(&t, &s)| t - s)
        .collect();
    let delta_db = center_midband(&f_src, &raw_diff);

    let (bands, rmse) = fit_peq_bands(
        &f_src,
        &delta_db,
        max_bands,
        max_freq_fit,
        max_gain_db,
        supports_shelves,
    );

    let src_ff = source_entry.form_factor();
    let tgt_ff = target_phone.form_factor();
    let src_rig = source_entry.rig_tag();
    let tgt_rig = target_phone.rig_tag();

    let warning = if src_ff != tgt_ff {
        Some(format!(
            "Aviso: comparando {src_ff} vs {tgt_ff}; el balance tonal relativo se mantiene, pero el oído percibe distinto over-ear vs IEM."
        ))
    } else if src_rig != tgt_rig {
        Some(format!(
            "Aviso: distinto equipo de medición ({src_rig} vs {tgt_rig}); pueden variar los agudos >6 kHz."
        ))
    } else {
        None
    };

    let target_fr_aligned = align_target_overlay(&f_src, &fr_src, &f_tgt, &fr_tgt, &delta_db);

    Ok(MatchResult {
        freqs: f_src,
        delta_db,
        stock_fr: fr_src,
        target_fr_aligned,
        bands,
        rmse,
        warning,
    })
}

pub fn compute_target_curve_peq(
    source_entry: &AutoEqEntry,
    target_csv_name: Option<&str>,
    max_freq_fit: f64,
    max_bands: usize,
    max_gain_db: f64,
    supports_shelves: bool,
) -> Result<MatchResult, String> {
    let ff = source_entry.form_factor();
    let (f_src, fr_src) = reconstruct_fr_curve(source_entry)?;
    let (f_tgt, fr_tgt) = reconstruct_target_csv_fr(target_csv_name, &ff)?;
    let fr_tgt_i = interp_log_freq(&f_src, &f_tgt, &fr_tgt);
    let raw_diff: Vec<f64> = fr_tgt_i
        .iter()
        .zip(fr_src.iter())
        .map(|(&t, &s)| t - s)
        .collect();
    let desired_eq = center_midband(&f_src, &raw_diff);
    let target_fr_aligned = align_target_overlay(&f_src, &fr_src, &f_tgt, &fr_tgt, &desired_eq);

    if source_entry.source_kind == "autoeq"
        && target_csv_name.unwrap_or("").is_empty()
        && max_bands >= 5
        && supports_shelves
        && max_gain_db >= 12.0
    {
        let cdir = cache_dir();
        let digest = sha1_hex(&source_entry.rel_path, 16);
        let cache_path = cdir.join("profiles").join(format!("{digest}_peq.txt"));
        if let Ok(txt) = fetch_text(&source_entry.parametric_eq_url(), &cache_path, false, 10) {
            let mut pre_bands: Vec<FilterBand> =
                parse_parametric_eq(&txt).into_iter().take(max_bands).collect();
            if pre_bands.len() >= max_bands.min(5)
                && pre_bands.iter().all(|b| b.gain.abs() <= max_gain_db)
            {
                let resp = combined_response_db(&pre_bands, &f_src, SAMPLE_RATE_HZ);
                let resp_mid = center_midband(&f_src, &resp);
                let w = make_weights(&f_src, max_freq_fit);
                let err: Vec<f64> = desired_eq
                    .iter()
                    .zip(resp_mid.iter())
                    .map(|(&d, &r)| d - r)
                    .collect();
                let pre_rmse = weighted_rmse(&err, &w);
                if pre_rmse < 0.75 {
                    let pad_freqs = [31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
                    while pre_bands.len() < max_bands {
                        pre_bands.push(FilterBand::new(
                            FilterType::Peaking,
                            pad_freqs[pre_bands.len() % pad_freqs.len()],
                            0.0,
                            1.0,
                        ));
                    }
                    return Ok(MatchResult {
                        freqs: f_src,
                        delta_db: desired_eq,
                        stock_fr: fr_src,
                        target_fr_aligned,
                        bands: pre_bands,
                        rmse: pre_rmse,
                        warning: None,
                    });
                }
            }
        }
    }

    let (bands, rmse) = fit_peq_bands(
        &f_src,
        &desired_eq,
        max_bands,
        max_freq_fit,
        max_gain_db,
        supports_shelves,
    );

    Ok(MatchResult {
        freqs: f_src,
        delta_db: desired_eq,
        stock_fr: fr_src,
        target_fr_aligned,
        bands,
        rmse,
        warning: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_punctuation_for_search() {
        assert_eq!(
            normalize_search_text("7Hz x Crinacle: Zero-3_IEM"),
            "7hz x crinacle zero 3 iem"
        );
    }

    #[test]
    fn parses_autoeq_index_and_parametric_eq() {
        let line = "- [Moondrop Chu 2](./oratory1990/in-ear/Moondrop%20Chu%202) by oratory1990";
        let entry = parse_autoeq_index_line(line).unwrap();
        assert_eq!(entry.name, "Moondrop Chu 2");
        assert_eq!(entry.form_factor(), "in-ear");
        assert_eq!(entry.rig_tag(), "711");

        let apo = "\
Preamp: -4.2 dB
Filter 1: ON LSC Fc 105 Hz Gain 4.2 dB Q 0.707
Filter 2: ON PK Fc 2850 Hz Gain -2.3 dB Q 1.85
Filter 3: ON HSC Fc 10000 Hz Gain 1.5 dB Q 0.707
";
        let bands = parse_parametric_eq(apo);
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].filter_type, FilterType::LowShelf);
        assert_eq!(bands[1].frequency, 2850.0);
        assert_eq!(bands[2].filter_type, FilterType::HighShelf);

        let exported = export_parametric_eq(&bands);
        let reparsed = parse_parametric_eq(&exported);
        assert_eq!(reparsed, bands);

        let apo_pre = export_parametric_eq_with_preamp(&bands, -4.2);
        assert!(apo_pre.starts_with("Preamp: -4.2 dB\n"));

        let ee_raw = export_easyeffects_json(&bands, -3.5);
        let ee_val: serde_json::Value = serde_json::from_str(&ee_raw).unwrap();
        assert_eq!(ee_val["output"]["equalizer#0"]["num-bands"], 3);
        assert_eq!(ee_val["output"]["equalizer#0"]["input-gain"], -3.5);
        assert_eq!(ee_val["output"]["equalizer#0"]["left"]["band0"]["type"], "Lo-shelf");

        let cdsp = export_camilladsp_yaml(&bands, -3.5);
        assert!(cdsp.contains("type: Gain"));
        assert!(cdsp.contains("gain: -3.50"));
        assert!(cdsp.contains("band_01:"));
        assert!(cdsp.contains("type: Lowshelf"));
        assert!(cdsp.contains("pipeline:"));

        let wavelet = export_wavelet_graphiceq(&bands);
        assert!(wavelet.starts_with("GraphicEQ: 20 "));
        let body = wavelet.trim().strip_prefix("GraphicEQ:").unwrap();
        let points: Vec<&str> = body.split(';').collect();
        assert_eq!(points.len(), 127);
        for pt in points {
            let vals: Vec<&str> = pt.trim().split_whitespace().collect();
            assert_eq!(vals.len(), 2);
            let db: f64 = vals[1].parse().unwrap();
            assert!(db <= 0.01, "Wavelet GraphicEQ must normalize peak <= 0 dB, got {db}");
        }
    }
}
