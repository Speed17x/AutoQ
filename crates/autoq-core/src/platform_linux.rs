//! Linux PipeWire 10-band native `filter-chain` PEQ, sink discovery, and
//! optional Classic Bluetooth SPP (RFCOMM) client.

use crate::biquad::{compute_safe_preamp_db, FilterBand, FilterType};
use crate::config::{bt_profiles_file, config_dir, load_saved_profile_bands, save_profile_bands, DEFAULT_VIRTUAL_BANDS};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub const PW_UNIT_NAME: &str = "moondrop-pw-eq";
pub const PW_SINK_NAME: &str = "effect_input.moondrop_peq";

const MOONDROP_BT_KEYWORDS: &[&str] = &[
    "moondrop",
    "space travel",
    "golden ages",
    "nekocake",
    "moca",
    "robin",
    "marigold",
    "kuroneko",
    "ultrasonic",
    "pill",
    "edge",
    "block",
    "alice",
];

const BT_AUTOEQ_HINTS: &[(&str, &str)] = &[
    ("space travel 2 ultra", "Moondrop Space Travel 2 Ultra"),
    ("space travel 2", "Moondrop Space Travel 2"),
    ("space travel", "Moondrop Space Travel"),
    ("golden ages 2", "Moondrop Golden Ages 2"),
    ("golden ages", "Moondrop Golden Ages"),
    ("moca", "Moondrop Moca"),
    ("nekocake", "Moondrop Nekocake"),
    ("edge", "Moondrop Edge"),
    ("block", "Moondrop BLOCK"),
    ("alice", "Moondrop Alice"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeWireAudioDevice {
    pub sink_name: String,
    pub mac: String,
    pub display_name: String,
    pub autoeq_hint: String,
    pub kind: String, // "bt_pipewire" | "usb_pipewire" | "wired_pipewire"
    pub transport_badge: String,
    pub supports_spp: bool,
    pub max_bands: usize,
    pub max_gain_db: f64,
    pub supports_shelves: bool,
}

fn is_moondrop_bt_name(name: &str) -> bool {
    let low = name.to_ascii_lowercase();
    MOONDROP_BT_KEYWORDS.iter().any(|k| low.contains(k))
}

fn infer_autoeq_hint(name: &str) -> String {
    let low = name.to_ascii_lowercase();
    for &(substr, hint) in BT_AUTOEQ_HINTS {
        if low.contains(substr) {
            return hint.to_string();
        }
    }
    let noise_words = [
        "le",
        "ble",
        "hands-free",
        "ag",
        "audio",
        "stereo",
        "estéreo",
        "estereo",
        "analógico",
        "analogico",
        "analog",
        "headset",
        "headphones",
        "earbuds",
        "controller",
        "hd-audio",
    ];
    let kept: Vec<&str> = name
        .split_whitespace()
        .filter(|w| {
            let wl = w
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
                .to_lowercase();
            !noise_words.contains(&wl.as_str())
        })
        .collect();
    kept.join(" ")
}

fn mac_from_sink_name(sink_name: &str) -> String {
    let Some(idx) = sink_name.find("bluez_output.") else {
        return String::new();
    };
    let rest = &sink_name[idx + "bluez_output.".len()..];
    if rest.len() >= 17 {
        let candidate = &rest[..17];
        if candidate
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'_' || b == b':')
        {
            return candidate.replace('_', ":").to_ascii_uppercase();
        }
    }
    String::new()
}

fn run_cmd(prog: &str, args: &[&str]) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("Solo disponible en Linux.".to_string());
    }
    let out = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("{prog}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn get_default_sink() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    run_cmd("pactl", &["get-default-sink"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn scan_pipewire_audio_sinks(usb_hid_vid_pids: &[String]) -> Vec<PipeWireAudioDevice> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    let Ok(out) = run_cmd("pactl", &["-f", "json", "list", "sinks"]) else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(sinks)) = serde_json::from_str::<serde_json::Value>(&out)
    else {
        return Vec::new();
    };

    let hid_vp_set: HashSet<String> = usb_hid_vid_pids
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut found = Vec::new();
    for s in sinks {
        let Some(obj) = s.as_object() else {
            continue;
        };
        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty()
            || name == PW_SINK_NAME
            || name.contains("auto_null")
            || name.to_ascii_lowercase().contains("dummy")
        {
            continue;
        }
        let props = obj.get("properties").and_then(|v| v.as_object());
        let prop_str = |key: &str| -> Option<&str> {
            props.and_then(|p| p.get(key)).and_then(|v| v.as_str())
        };
        let desc = obj
            .get("description")
            .and_then(|v| v.as_str())
            .or_else(|| prop_str("device.description"))
            .or_else(|| prop_str("node.description"))
            .or_else(|| prop_str("device.alias"))
            .unwrap_or(name)
            .trim()
            .to_string();

        if name.starts_with("bluez_output.") {
            let mac = mac_from_sink_name(name);
            found.push(PipeWireAudioDevice {
                sink_name: name.to_string(),
                mac,
                autoeq_hint: infer_autoeq_hint(&desc),
                supports_spp: is_moondrop_bt_name(&desc) || is_moondrop_bt_name(name),
                display_name: desc,
                kind: "bt_pipewire".to_string(),
                transport_badge: "BT·10B".to_string(),
                max_bands: DEFAULT_VIRTUAL_BANDS,
                max_gain_db: 12.0,
                supports_shelves: true,
            });
            continue;
        }

        if name.starts_with("alsa_output.usb-") {
            let vid = prop_str("device.vendor.id")
                .unwrap_or("")
                .to_ascii_lowercase()
                .replace("0x", "");
            let pid = prop_str("device.product.id")
                .unwrap_or("")
                .to_ascii_lowercase()
                .replace("0x", "");
            let vp = if !vid.is_empty() && !pid.is_empty() {
                format!("{vid}:{pid}")
            } else {
                String::new()
            };
            if !vp.is_empty() && hid_vp_set.contains(&vp) {
                continue;
            }
            if hid_vp_set.contains("31b2:0113")
                && (desc.to_ascii_lowercase().contains("chu2")
                    || name.to_ascii_lowercase().contains("chu2"))
            {
                continue;
            }
            found.push(PipeWireAudioDevice {
                sink_name: name.to_string(),
                mac: String::new(),
                autoeq_hint: infer_autoeq_hint(&desc),
                display_name: desc,
                kind: "usb_pipewire".to_string(),
                transport_badge: "USB·10B".to_string(),
                supports_spp: false,
                max_bands: DEFAULT_VIRTUAL_BANDS,
                max_gain_db: 12.0,
                supports_shelves: true,
            });
            continue;
        }

        if name.starts_with("alsa_output.") && !name.to_ascii_lowercase().contains("hdmi") {
            found.push(PipeWireAudioDevice {
                sink_name: name.to_string(),
                mac: String::new(),
                display_name: desc,
                autoeq_hint: String::new(),
                kind: "wired_pipewire".to_string(),
                transport_badge: "AUX·10B".to_string(),
                supports_spp: false,
                max_bands: DEFAULT_VIRTUAL_BANDS,
                max_gain_db: 12.0,
                supports_shelves: true,
            });
        }
    }

    let rank = |k: &str| match k {
        "bt_pipewire" => 0,
        "usb_pipewire" => 1,
        "wired_pipewire" => 2,
        _ => 9,
    };
    found.sort_by(|a, b| {
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    found
}

pub fn is_pipewire_eq_running() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    let unit = format!("{PW_UNIT_NAME}.service");
    run_cmd("systemctl", &["--user", "is-active", &unit])
        .map(|s| s.trim() == "active")
        .unwrap_or(false)
}

pub fn load_pw_saved_bands(dev: &PipeWireAudioDevice) -> Vec<FilterBand> {
    let key = if !dev.mac.is_empty() {
        &dev.mac
    } else if !dev.sink_name.is_empty() {
        &dev.sink_name
    } else {
        &dev.display_name
    };
    load_saved_profile_bands(&bt_profiles_file(), key, dev.max_bands)
}

pub fn build_pipewire_conf(bands: &[FilterBand], target_sink: &str, label: &str) -> String {
    build_pipewire_conf_with_preamp(bands, target_sink, label, compute_safe_preamp_db(bands))
}

pub fn build_pipewire_conf_with_preamp(
    bands: &[FilterBand],
    target_sink: &str,
    label: &str,
    preamp_db: f64,
) -> String {
    let mut active: Vec<FilterBand> = bands.iter().map(FilterBand::clamped).collect();
    if active.is_empty() {
        active.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
    }

    let safe_preamp = if preamp_db.is_finite() {
        preamp_db.clamp(-18.0, 6.0)
    } else {
        0.0
    };
    let use_preamp = safe_preamp.abs() >= 0.05;

    let mut nodes_lines = Vec::with_capacity(active.len() + 1);
    let mut links_lines = Vec::with_capacity(active.len() + 1);

    if use_preamp {
        let linear = 10.0_f64.powf(safe_preamp / 20.0);
        nodes_lines.push(format!(
            "                    {{ type = builtin name = eq_preamp label = mixer control = {{ \"Gain 1\" = {linear:.6} }} }}"
        ));
        links_lines.push(
            "                    { output = \"eq_preamp:Out\" input = \"eq_band_1:In\" }"
                .to_string(),
        );
    }

    for (idx, b) in active.iter().enumerate() {
        let i = idx + 1;
        nodes_lines.push(format!(
            "                    {{ type = builtin name = eq_band_{i} label = {} control = {{ \"Freq\" = {:.1} \"Q\" = {:.3} \"Gain\" = {:.1} }} }}",
            b.filter_type.pipewire_label(),
            b.frequency,
            b.q,
            b.gain
        ));
        if i > 1 {
            links_lines.push(format!(
                "                    {{ output = \"eq_band_{}:Out\" input = \"eq_band_{i}:In\" }}",
                i - 1
            ));
        }
    }

    let nodes_block = nodes_lines.join("\n");
    let links_block = if links_lines.is_empty() {
        String::new()
    } else {
        format!(
            "                links = [\n{}\n                ]",
            links_lines.join("\n")
        )
    };
    let io_block = if use_preamp {
        format!(
            "\n                inputs  = [ \"eq_preamp:In 1\" ]\n                outputs = [ \"eq_band_{}:Out\" ]",
            active.len()
        )
    } else {
        String::new()
    };

    let safe_desc = label.replace('"', "");
    let safe_target = target_sink.replace('"', "");
    format!(
        "context.properties = {{ log.level = 0 }}
context.spa-libs = {{
    audio.convert.* = audioconvert/libspa-audioconvert
    support.*       = support/libspa-support
}}
context.modules = [
    {{ name = libpipewire-module-protocol-native }}
    {{ name = libpipewire-module-client-node }}
    {{ name = libpipewire-module-adapter }}
    {{ name = libpipewire-module-filter-chain
        args = {{
            node.description = \"AutoQ PEQ ({safe_desc})\"
            media.name       = \"AutoQ PEQ ({safe_desc})\"
            filter.graph = {{
                nodes = [
{nodes_block}
                ]
{links_block}{io_block}
            }}
            audio.channels = 2
            audio.position = [ FL FR ]
            capture.props = {{
                node.name   = \"{PW_SINK_NAME}\"
                media.class = Audio/Sink
            }}
            playback.props = {{
                node.name     = \"effect_output.moondrop_peq\"
                target.object = \"{safe_target}\"
                node.passive  = true
            }}
        }}
    }}
]
"
    )
}

pub fn stop_pipewire_eq(restore_sink: Option<&str>) {
    if !cfg!(target_os = "linux") {
        return;
    }
    let unit = format!("{PW_UNIT_NAME}.service");
    let _ = run_cmd("systemctl", &["--user", "stop", &unit]);
    let _ = run_cmd("systemctl", &["--user", "reset-failed", &unit]);

    if let Some(sink) = restore_sink {
        let _ = run_cmd("pactl", &["set-default-sink", sink]);
        if let Ok(out) = run_cmd("pactl", &["-f", "json", "list", "sink-inputs"]) {
            if let Ok(serde_json::Value::Array(inputs)) = serde_json::from_str(&out) {
                for si in inputs {
                    if let Some(idx) = si.get("index").and_then(|v| v.as_u64()) {
                        let idx_s = idx.to_string();
                        let _ = run_cmd("pactl", &["move-sink-input", &idx_s, sink]);
                    }
                }
            }
        }
    }
}

pub fn apply_pipewire_eq(
    dev: &PipeWireAudioDevice,
    bands: &[FilterBand],
    enable_eq: bool,
) -> Result<bool, String> {
    apply_pipewire_eq_with_preamp(dev, bands, enable_eq, None)
}

pub fn apply_pipewire_eq_with_preamp(
    dev: &PipeWireAudioDevice,
    bands: &[FilterBand],
    enable_eq: bool,
    preamp_db: Option<f64>,
) -> Result<bool, String> {
    let key = if !dev.mac.is_empty() {
        &dev.mac
    } else if !dev.sink_name.is_empty() {
        &dev.sink_name
    } else {
        &dev.display_name
    };
    let _ = save_profile_bands(&bt_profiles_file(), key, bands);

    let effective_preamp = preamp_db.unwrap_or_else(|| compute_safe_preamp_db(bands));
    if !enable_eq || (bands.iter().all(|b| b.gain.abs() < 0.05) && effective_preamp.abs() < 0.05) {
        stop_pipewire_eq(Some(&dev.sink_name));
        return Ok(false);
    }

    let cdir = config_dir();
    fs::create_dir_all(&cdir).map_err(|e| e.to_string())?;
    let conf_path = cdir.join("pw-moondrop-eq.conf");
    fs::write(
        &conf_path,
        build_pipewire_conf_with_preamp(bands, &dev.sink_name, &dev.display_name, effective_preamp),
    )
    .map_err(|e| e.to_string())?;

    let unit_svc = format!("{PW_UNIT_NAME}.service");
    let _ = run_cmd("systemctl", &["--user", "stop", &unit_svc]);
    let _ = run_cmd("systemctl", &["--user", "reset-failed", &unit_svc]);

    let unit_arg = format!("--unit={PW_UNIT_NAME}");
    let conf_str = conf_path.to_string_lossy().into_owned();
    run_cmd(
        "systemd-run",
        &[
            "--user",
            &unit_arg,
            "--description=AutoQ PipeWire PEQ Filter",
            "--collect",
            "pipewire",
            "-c",
            &conf_str,
        ],
    )
    .map_err(|e| format!("No se pudo iniciar PipeWire EQ: {e}"))?;

    for _ in 0..15 {
        if let Ok(out_s) = run_cmd("pactl", &["list", "sinks", "short"]) {
            if out_s.contains(PW_SINK_NAME) {
                break;
            }
        }
        thread::sleep(Duration::from_millis(80));
    }

    let _ = run_cmd("pactl", &["set-default-sink", PW_SINK_NAME]);
    if let Ok(out_i) = run_cmd("pactl", &["-f", "json", "list", "sink-inputs"]) {
        if let Ok(serde_json::Value::Array(inputs)) = serde_json::from_str(&out_i) {
            for si in inputs {
                if let Some(idx) = si.get("index").and_then(|v| v.as_u64()) {
                    let idx_s = idx.to_string();
                    let _ = run_cmd("pactl", &["move-sink-input", &idx_s, PW_SINK_NAME]);
                }
            }
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Classic Bluetooth SPP (RFCOMM) for compatible Moondrop TWS (Linux AF_BLUETOOTH)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod spp {
    use super::*;
    use crate::posix_fd::PosixFd;
    use std::time::Instant;

    fn parse_bdaddr_le(mac: &str) -> Result<[u8; 6], String> {
        let parts: Vec<&str> = mac.trim().split(':').collect();
        if parts.len() != 6 {
            return Err(format!("Dirección MAC Bluetooth inválida: {mac}"));
        }
        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            // BlueZ bdaddr_t stores octets in reverse (little-endian) order
            bytes[5 - i] = u8::from_str_radix(part, 16)
                .map_err(|_| format!("Octeto MAC inválido en {mac}"))?;
        }
        Ok(bytes)
    }

    fn resolve_spp_channels(mac: &str) -> Vec<u8> {
        let mut channels = Vec::new();
        if let Ok(out) = run_cmd("sdptool", &["browse", mac]) {
            for blk in out.split("Service Name:") {
                if blk.contains("0x1101") || blk.contains("Serial Port") {
                    for line in blk.lines() {
                        if let Some(rest) = line.trim().strip_prefix("Channel:") {
                            if let Ok(ch) = rest.trim().parse::<u8>() {
                                if !channels.contains(&ch) {
                                    channels.push(ch);
                                }
                            }
                        }
                    }
                }
            }
        }
        for ch in 1..=5 {
            if !channels.contains(&ch) {
                channels.push(ch);
            }
        }
        channels
    }

    fn spp_frame(payload: &[u8]) -> Vec<u8> {
        let n = payload.len().saturating_sub(4);
        let mut out = Vec::with_capacity(4 + payload.len());
        out.push(0xFF);
        out.push(0x04);
        out.push(((n >> 8) & 0xFF) as u8);
        out.push((n & 0xFF) as u8);
        out.extend_from_slice(payload);
        out
    }

    pub fn spp_request(mac: &str, payload: &[u8], timeout_ms: i32) -> Result<Vec<u8>, String> {
        if mac.trim().is_empty() {
            return Err("No hay dirección MAC Bluetooth para SPP.".to_string());
        }
        let bdaddr = parse_bdaddr_le(mac)?;
        let mut conn_fd: Option<PosixFd> = None;
        let mut last_err = String::new();

        for ch in resolve_spp_channels(mac) {
            let sock = match PosixFd::open_rfcomm_socket(timeout_ms) {
                Ok(s) => s,
                Err(e) => {
                    last_err = e.to_string();
                    continue;
                }
            };
            match sock.connect_rfcomm(bdaddr, ch) {
                Ok(()) => {
                    conn_fd = Some(sock);
                    break;
                }
                Err(e) => last_err = e.to_string(),
            }
        }

        let sock = conn_fd
            .ok_or_else(|| format!("No se pudo conectar por Bluetooth SPP ({last_err})."))?;

        let frame_out = spp_frame(payload);
        sock.send(&frame_out)
            .map_err(|e| format!("Error enviando trama SPP: {e}"))?;

        let want = payload.get(2).copied().unwrap_or(0) & 0xFE;
        let mut buf: Vec<u8> = Vec::with_capacity(256);
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);

        while Instant::now() < deadline {
            let mut chunk = [0u8; 512];
            let Ok(n) = sock.recv(&mut chunk) else {
                break;
            };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);

            let mut i = 0usize;
            while i + 8 <= buf.len() {
                if buf[i] == 0xFF && buf[i + 1] == 0x04 {
                    let ln = ((buf[i + 2] as usize) << 8) | (buf[i + 3] as usize);
                    let total = 8 + ln;
                    if buf.len() - i >= total {
                        let frame = &buf[i..i + total];
                        if (frame[6] & 0xFE) == want {
                            return Ok(frame.to_vec());
                        }
                        i += total;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
        }
        Err("Tiempo de espera agotado esperando respuesta Bluetooth SPP.".to_string())
    }
}

#[cfg(not(target_os = "linux"))]
mod spp {
    pub fn spp_request(_mac: &str, _payload: &[u8], _timeout_ms: i32) -> Result<Vec<u8>, String> {
        Err("Bluetooth SPP directo solo está soportado en Linux.".to_string())
    }
}

pub fn bt_spp_set_preset(mac: &str, preset_idx: u8) -> Result<(), String> {
    spp::spp_request(mac, &[0x00, 0x1D, 0x0A, 0x03, preset_idx], 1800).map(|_| ())
}

pub fn bt_spp_read_eq(mac: &str) -> Result<Vec<FilterBand>, String> {
    let frame = spp::spp_request(mac, &[0x00, 0x1D, 0x0A, 0x05], 1800)?;
    let idx = frame
        .windows(2)
        .enumerate()
        .skip(6)
        .find(|(_, w)| *w == [0x00, 0x04])
        .map(|(i, _)| i)
        .ok_or_else(|| {
            "Este modelo Bluetooth no devolvió una curva PEQ personalizable por SPP.".to_string()
        })?;
    let p = &frame[idx..];
    if p.len() < 11 || (p.len() - 4) % 7 != 0 {
        return Err("Formato EQ SPP no reconocido para este modelo.".to_string());
    }
    let n = (p.len() - 4) / 7;
    let s16 = |o: usize| i16::from_be_bytes([p[o], p[o + 1]]);
    let u16_be = |o: usize| u16::from_be_bytes([p[o], p[o + 1]]);

    let mut bands = Vec::with_capacity(n);
    for i in 0..n {
        let o = 2 + i * 7;
        let g_raw = if i + 1 < n {
            s16(o + 7)
        } else {
            s16(2 + n * 7)
        };
        bands.push(
            FilterBand::new(
                FilterType::Peaking,
                u16_be(o + 2) as f64,
                ((g_raw as f64 / 60.0) * 10.0).round() / 10.0,
                ((u16_be(o + 4) as f64 / 4096.0) * 1000.0).round() / 1000.0,
            )
            .clamped(),
        );
    }
    Ok(bands)
}

pub fn bt_spp_write_eq(mac: &str, bands: &[FilterBand]) -> Result<(), String> {
    let mut five: Vec<FilterBand> = bands.iter().take(5).map(FilterBand::clamped).collect();
    while five.len() < 5 {
        five.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
    }
    let max_pos = five
        .iter()
        .map(|b| b.gain)
        .filter(|&g| g > 0.0)
        .fold(0.0_f64, f64::max);
    let preamp = -max_pos;

    let mut out = vec![0u8; 4 + five.len() * 7];
    out[0] = 0x00;
    out[1] = 0x04;

    let put_s16 = |buf: &mut [u8], o: usize, val: f64| {
        let r = (val * 60.0).round().clamp(-32768.0, 32767.0) as i16;
        buf[o..o + 2].copy_from_slice(&r.to_be_bytes());
    };
    let put_u16 = |buf: &mut [u8], o: usize, val: i64| {
        let r = val.clamp(0, 65535) as u16;
        buf[o..o + 2].copy_from_slice(&r.to_be_bytes());
    };

    put_s16(&mut out, 2, preamp);
    for (i, b) in five.iter().enumerate() {
        let o = 2 + i * 7;
        put_s16(&mut out, o, if i == 0 { preamp } else { five[i - 1].gain });
        put_u16(&mut out, o + 2, b.frequency.round() as i64);
        put_u16(&mut out, o + 4, (b.q * 4096.0).round() as i64);
        out[o + 6] = 0;
    }
    put_s16(&mut out, 2 + five.len() * 7, five.last().unwrap().gain);

    for slot in [0x03u8, 0xFFu8] {
        if spp::spp_request(mac, &[0x00, 0x1D, 0x0A, 0x03, slot], 800).is_ok() {
            break;
        }
    }
    let mut write_payload = vec![0x00, 0x1D, 0x0A, 0x06];
    write_payload.extend_from_slice(&out);
    spp::spp_request(mac, &write_payload, 2000).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_pipewire_conf_with_and_without_preamp_mixer() {
        let bands = vec![FilterBand::new(FilterType::Peaking, 1000.0, 4.0, 1.0)];
        let conf_pre = build_pipewire_conf_with_preamp(&bands, "alsa_output.test", "Test", -6.0);
        assert!(conf_pre.contains("name = eq_preamp label = mixer"));
        assert!(conf_pre.contains("\"Gain 1\" = 0.5011"));
        assert!(conf_pre.contains("output = \"eq_preamp:Out\" input = \"eq_band_1:In\""));
        assert!(conf_pre.contains("inputs  = [ \"eq_preamp:In 1\" ]"));

        let conf_zero = build_pipewire_conf_with_preamp(&bands, "alsa_output.test", "Test", 0.0);
        assert!(!conf_zero.contains("eq_preamp"));
    }
}
