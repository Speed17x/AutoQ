//! Unified cross-platform multiprotocol USB-C HID DSP engine for AutoQ:
//! 1. KTMicro (`VID 0x31B2`): Moondrop Chu 2 DSP / CDSP / Quark 2 / Jiu / May,
//!    Kiwi Ears Allegro Mini/PRO, JCALLY JM12/DSP, Tanchjim Zero/One/Tanya DSP.
//! 2. Moondrop 8-band: FreeDSP Pro/Mini, Rays, AG Rays, Marigold, DAWN PRO 2,
//!    Echo A, MOONRIVER 3, DHA15.
//! 3. WalkPlay (19 VIDs, 5–10 bands): Tanchjim Bunny/Stargate II/Space Pro,
//!    CrinEar Protocol Max, NiceHCK, LETSHUOER, ddHifi, Truthear DSP dongles.
//! 4. FiiO / JadeAudio / SNOWSKY (`VID 0x2972`, `0x0A12`, 5–10 bands):
//!    JA11, JIEZI, KA15, KA17, BTR13, BTR17, QX13, Q7, BT11, SNOWSKY Melody/TINY.
//!
//! Protocol definitions cross-referenced with open-source `jeromeof/devicePEQ` (0BSD).

use crate::biquad::{compute_safe_preamp_db, FilterBand, FilterType};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::thread;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Protocol Constants
// ---------------------------------------------------------------------------

// KTMicro 5-band constants
pub const KT_VID: u16 = 0x31B2;
pub const CHU2_DSP_PID: u16 = 0x0113;
pub const KT_REPORT_ID: u8 = 0x4B;
pub const KT_READ: u8 = 0x52;
pub const KT_WRITE: u8 = 0x57;
pub const KT_COMMIT: u8 = 0x53;
pub const KT_REG_SLOT: u8 = 0x24;
pub const KT_REG_BAND0: u8 = 0x26;
pub const KT_SLOT_ON: u8 = 0x03;
pub const KT_SLOT_OFF: u8 = 0x02;

// Moondrop 8-band & WalkPlay constants (report ID 0x4B)
pub const MD_REPORT_ID: u8 = 0x4B;
pub const MD_CMD_WRITE: u8 = 0x01;
pub const MD_CMD_READ: u8 = 0x80;
pub const MD_CMD_SAVE_FLASH: u8 = 0x01;
pub const WP_CMD_GLOBAL_GAIN: u8 = 0x03;
pub const MD_CMD_UPDATE_EQ: u8 = 0x09;
pub const MD_CMD_APPLY_COEFF: u8 = 0x0A;

// FiiO / JadeAudio / SNOWSKY HID constants
pub const FIIO_VID_PRIMARY: u16 = 0x2972;
pub const FIIO_VID_QUALCOMM: u16 = 0x0A12;
pub const FIIO_SET_H1: u8 = 0xAA;
pub const FIIO_SET_H2: u8 = 0x0A;
pub const FIIO_GET_H1: u8 = 0xBB;
pub const FIIO_GET_H2: u8 = 0x0B;
pub const FIIO_END: u8 = 0xEE;
pub const FIIO_CMD_FILTER_PARAMS: u8 = 0x15;
pub const FIIO_CMD_PRESET_SWITCH: u8 = 0x16;
pub const FIIO_CMD_GLOBAL_GAIN: u8 = 0x17;
pub const FIIO_CMD_FILTER_COUNT: u8 = 0x18;
pub const FIIO_CMD_SAVE_TO_DEVICE: u8 = 0x19;
pub const FIIO_CMD_SAVE_V2: u8 = 0x21;

pub const WALKPLAY_VIDS: &[u16] = &[
    0x0104, 0x011B, 0x011D, 0x0661, 0x0663, 0x0666, 0x0762, 0x0909, 0x0D8C, 0x2FC6, 0x3302,
    0x34BE, 0x35D8, 0x36A7, 0x373B, 0x60C1, 0x60E1, 0xB445, 0xB44D,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HidProtocol {
    #[serde(rename = "ktmicro")]
    KtMicro,
    #[serde(rename = "moondrop8")]
    Moondrop8,
    #[serde(rename = "walkplay")]
    WalkPlay,
    #[serde(rename = "fiio")]
    Fiio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoondropDeviceInfo {
    pub hidraw_path: String,
    pub vid: u16,
    pub pid: u16,
    pub hid_name: String,
    pub display_name: String,
    pub protocol: HidProtocol,
    pub max_bands: usize,
    pub max_gain_db: f64,
    pub supports_shelves: bool,
    pub disconnects_on_commit: bool,
    pub autoeq_hint: String,
    #[serde(default = "default_report_id")]
    pub report_id: u8,
    #[serde(default = "default_user_slot")]
    pub user_slot: u8,
    #[serde(default = "default_bypass_slot")]
    pub bypass_slot: u8,
    #[serde(default)]
    pub fiio_save_v2: bool,
}

fn default_report_id() -> u8 {
    KT_REPORT_ID
}
fn default_user_slot() -> u8 {
    7
}
fn default_bypass_slot() -> u8 {
    240
}

impl MoondropDeviceInfo {
    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", self.vid, self.pid)
    }

    pub fn protocol_badge(&self) -> String {
        let p = match self.protocol {
            HidProtocol::KtMicro => "HID",
            HidProtocol::Moondrop8 => "MD",
            HidProtocol::WalkPlay => "WP",
            HidProtocol::Fiio => "FIIO",
        };
        format!("{p}·{}B", self.max_bands)
    }
}

/// Raw HID entry returned by `scan_all_hid_devices()` for diagnostics (`autoq-cli hid-scan`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHidDevice {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub vid_pid: String,
    pub hid_name: String,
    pub recognized: bool,
    pub matched_name: Option<String>,
    pub protocol: Option<HidProtocol>,
}

const MOONDROP8_SPECS: &[(&str, &str, f64, bool, &str)] = &[
    ("ag rays", "Moondrop AG Rays", 12.0, false, "Moondrop"),
    ("rays", "Moondrop Rays", 12.0, false, "Moondrop"),
    ("marigold", "Moondrop Marigold", 6.0, false, "Moondrop"),
    ("freedsp pro", "Moondrop FreeDSP Pro", 12.0, true, "Moondrop"),
    ("freedsp mini", "Moondrop FreeDSP Mini", 12.0, true, "Moondrop"),
    ("dawn pro2", "Moondrop DAWN PRO 2", 12.0, true, "Moondrop"),
    ("dawn pro 2", "Moondrop DAWN PRO 2", 12.0, true, "Moondrop"),
    ("echo a", "Moondrop Echo A", 12.0, false, "Moondrop"),
    ("moonriver 3", "Moondrop MOONRIVER 3", 12.0, true, "Moondrop"),
    ("dha15", "Moondrop DHA15", 12.0, true, "Moondrop"),
];

// (match_substr, display_name, max_bands, max_gain, report_id, user_slot, bypass_slot, disconnect, save_v2, hint)
const FIIO_SPECS: &[(&str, &str, usize, f64, u8, u8, u8, bool, bool, &str)] = &[
    ("ja11", "JadeAudio JA11", 5, 12.0, 2, 3, 4, true, false, "JadeAudio JA11"),
    ("jiezi", "JadeAudio JIEZI", 5, 12.0, 2, 160, 240, false, false, "JadeAudio"),
    ("ka17", "FiiO KA17", 10, 12.0, 1, 7, 10, false, false, "FiiO"),
    ("ka15", "FiiO KA15", 10, 12.0, 7, 160, 10, false, false, "FiiO"),
    ("qx13", "FiiO QX13", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
    ("btr17", "FiiO BTR17", 10, 12.0, 7, 160, 240, false, true, "FiiO"),
    ("btr13", "FiiO BTR13", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
    ("bt11", "FiiO BT11", 10, 12.0, 7, 7, 11, false, false, "FiiO"),
    ("q7", "FiiO Q7", 10, 12.0, 7, 7, 10, false, false, "FiiO"),
    ("snowsky melody", "SNOWSKY Melody", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
    ("snowsky tiny", "SNOWSKY TINY", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
];

// (match_substr, display_name, max_bands, max_gain, shelves, slot, hint)
const WALKPLAY_SPECS: &[(&str, &str, usize, f64, bool, u8, &str)] = &[
    ("protocol max", "CrinEar Protocol Max", 10, 10.0, true, 101, "CrinEar"),
    ("stargate", "Tanchjim Stargate II", 8, 12.0, true, 101, "Tanchjim"),
    ("bunny", "Tanchjim Bunny DSP", 5, 12.0, true, 101, "Tanchjim Bunny"),
    ("space pro", "Tanchjim Space Pro", 8, 12.0, true, 101, "Tanchjim"),
    ("tanchjim", "Tanchjim USB DSP", 5, 12.0, true, 101, "Tanchjim"),
    ("nicehck", "NiceHCK USB DSP", 8, 12.0, true, 101, "NiceHCK"),
    ("letshuoer", "LETSHUOER USB DSP", 8, 12.0, true, 101, "LETSHUOER"),
    ("ddhifi", "ddHifi USB DSP", 8, 12.0, true, 101, "ddHifi"),
    ("truthear", "Truthear USB DSP", 8, 12.0, true, 101, "Truthear"),
    ("walkplay", "WalkPlay USB DSP", 8, 10.0, true, 101, ""),
];

pub fn classify_hid_device(
    dev_path: &str,
    vid: u16,
    pid: u16,
    hid_name: &str,
) -> Option<MoondropDeviceInfo> {
    let low = hid_name.to_ascii_lowercase();

    // 1. KTMicro 5-band family (VID 0x31B2): Moondrop, Kiwi Ears, JCALLY, Tanchjim
    if vid == KT_VID {
        let (display_name, hint) = if pid == CHU2_DSP_PID || low.contains("chu2") || low.contains("chu 2") {
            ("Moondrop Chu 2 DSP".to_string(), "Moondrop Chu 2".to_string())
        } else if low.contains("quark") {
            ("Moondrop Quark 2 DSP".to_string(), "Moondrop".to_string())
        } else if low.contains("jiu") {
            ("Moondrop Jiu DSP".to_string(), "Moondrop Jiu".to_string())
        } else if low.contains("may") {
            ("Moondrop May DSP".to_string(), "Moondrop May".to_string())
        } else if low.contains("kiwi") || low.contains("allegro") {
            let disp = if hid_name.trim().is_empty() {
                "Kiwi Ears Allegro DSP".to_string()
            } else {
                hid_name.trim().to_string()
            };
            (disp, "Kiwi Ears".to_string())
        } else if low.contains("jcally") || low.contains("jm12") {
            let disp = if hid_name.trim().is_empty() {
                "JCALLY USB DSP".to_string()
            } else {
                hid_name.trim().to_string()
            };
            (disp, "".to_string())
        } else if low.contains("tanchjim") || low.contains("tanya") || low.contains("zero") {
            let disp = if hid_name.trim().is_empty() {
                "Tanchjim USB DSP".to_string()
            } else {
                hid_name.trim().to_string()
            };
            (disp, "Tanchjim".to_string())
        } else if low.contains("cdsp") || low.contains("moondrop") || cfg!(target_os = "windows") || !low.is_empty() {
            let disp = if hid_name.trim().is_empty() {
                format!("KTMicro DSP ({vid:04x}:{pid:04x})")
            } else {
                hid_name.trim().to_string()
            };
            let hint = if low.contains("moondrop") { "Moondrop" } else { "" };
            (disp, hint.to_string())
        } else {
            return None;
        };

        return Some(MoondropDeviceInfo {
            hidraw_path: dev_path.to_string(),
            vid,
            pid,
            hid_name: hid_name.to_string(),
            display_name,
            protocol: HidProtocol::KtMicro,
            max_bands: 5,
            max_gain_db: 12.0,
            supports_shelves: true,
            disconnects_on_commit: true,
            autoeq_hint: hint,
            report_id: KT_REPORT_ID,
            user_slot: KT_SLOT_ON,
            bypass_slot: KT_SLOT_OFF,
            fiio_save_v2: false,
        });
    }

    // 2. Moondrop 8-band HID devices
    for &(substr, disp, max_g, shelves, hint) in MOONDROP8_SPECS {
        if low.contains(substr) {
            return Some(MoondropDeviceInfo {
                hidraw_path: dev_path.to_string(),
                vid,
                pid,
                hid_name: hid_name.to_string(),
                display_name: disp.to_string(),
                protocol: HidProtocol::Moondrop8,
                max_bands: 8,
                max_gain_db: max_g,
                supports_shelves: shelves,
                disconnects_on_commit: false,
                autoeq_hint: hint.to_string(),
                report_id: MD_REPORT_ID,
                user_slot: 7,
                bypass_slot: 0,
                fiio_save_v2: false,
            });
        }
    }

    // 3. FiiO / JadeAudio / SNOWSKY USB HID PEQ family
    if vid == FIIO_VID_PRIMARY
        || (vid == FIIO_VID_QUALCOMM && (low.contains("fiio") || low.contains("jade") || low.contains("snowsky")))
    {
        for &(substr, disp, max_b, max_g, rep_id, u_slot, b_slot, disc, save_v2, hint) in FIIO_SPECS {
            if low.contains(substr) {
                return Some(MoondropDeviceInfo {
                    hidraw_path: dev_path.to_string(),
                    vid,
                    pid,
                    hid_name: hid_name.to_string(),
                    display_name: disp.to_string(),
                    protocol: HidProtocol::Fiio,
                    max_bands: max_b,
                    max_gain_db: max_g,
                    supports_shelves: true,
                    disconnects_on_commit: disc,
                    autoeq_hint: hint.to_string(),
                    report_id: rep_id,
                    user_slot: u_slot,
                    bypass_slot: b_slot,
                    fiio_save_v2: save_v2,
                });
            }
        }
        if vid == FIIO_VID_PRIMARY {
            let disp = if hid_name.trim().is_empty() {
                format!("FiiO USB DSP ({vid:04x}:{pid:04x})")
            } else {
                hid_name.trim().to_string()
            };
            return Some(MoondropDeviceInfo {
                hidraw_path: dev_path.to_string(),
                vid,
                pid,
                hid_name: hid_name.to_string(),
                display_name: disp,
                protocol: HidProtocol::Fiio,
                max_bands: 10,
                max_gain_db: 12.0,
                supports_shelves: true,
                disconnects_on_commit: false,
                autoeq_hint: "FiiO".to_string(),
                report_id: 7,
                user_slot: 160,
                bypass_slot: 240,
                fiio_save_v2: false,
            });
        }
    }

    // 4. WalkPlay DSP chipset family (19 vendor IDs)
    if WALKPLAY_VIDS.contains(&vid) {
        // Explicit CrinEar Protocol Max PID (0x3302:0x43CC)
        if vid == 0x3302 && pid == 0x43CC {
            return Some(MoondropDeviceInfo {
                hidraw_path: dev_path.to_string(),
                vid,
                pid,
                hid_name: hid_name.to_string(),
                display_name: "CrinEar Protocol Max".to_string(),
                protocol: HidProtocol::WalkPlay,
                max_bands: 10,
                max_gain_db: 10.0,
                supports_shelves: true,
                disconnects_on_commit: false,
                autoeq_hint: "CrinEar".to_string(),
                report_id: MD_REPORT_ID,
                user_slot: 101,
                bypass_slot: 0,
                fiio_save_v2: false,
            });
        }

        for &(substr, disp, max_b, max_g, shelves, slot, hint) in WALKPLAY_SPECS {
            if low.contains(substr) {
                return Some(MoondropDeviceInfo {
                    hidraw_path: dev_path.to_string(),
                    vid,
                    pid,
                    hid_name: hid_name.to_string(),
                    display_name: disp.to_string(),
                    protocol: HidProtocol::WalkPlay,
                    max_bands: max_b,
                    max_gain_db: max_g,
                    supports_shelves: shelves,
                    disconnects_on_commit: false,
                    autoeq_hint: hint.to_string(),
                    report_id: MD_REPORT_ID,
                    user_slot: slot,
                    bypass_slot: 0,
                    fiio_save_v2: false,
                });
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Binary Protocol Codecs
// ---------------------------------------------------------------------------

// --- 1. KTMicro (5-band, little-endian registers) ---

pub fn kt_type_to_code(ft: FilterType) -> u8 {
    match ft {
        FilterType::Peaking => 0,
        FilterType::LowShelf => 3,
        FilterType::HighShelf => 4,
    }
}

pub fn kt_code_to_type(code: u8) -> FilterType {
    match code {
        3 => FilterType::LowShelf,
        4 => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

pub fn kt_packet(reg: u8, cmd: u8, value: [u8; 4]) -> [u8; 11] {
    [
        KT_REPORT_ID,
        reg,
        0,
        0,
        0,
        cmd,
        0,
        value[0],
        value[1],
        value[2],
        value[3],
    ]
}

pub fn kt_encode_band(band: &FilterBand) -> ([u8; 4], [u8; 4]) {
    let b = band.clamped();
    let g_i16 = (b.gain * 10.0).round() as i16;
    let f_u16 = b.frequency.round() as u16;
    let q_u16 = (b.q * 1000.0).round() as u16;

    let gb = g_i16.to_le_bytes();
    let fb = f_u16.to_le_bytes();
    let qb = q_u16.to_le_bytes();

    let gain_freq = [gb[0], gb[1], fb[0], fb[1]];
    let q_type = [qb[0], qb[1], kt_type_to_code(b.filter_type), 0];
    (gain_freq, q_type)
}

pub fn kt_decode_band(gain_freq: [u8; 4], q_type: [u8; 4]) -> FilterBand {
    let ftype = kt_code_to_type(q_type[2]);
    let gain = (i16::from_le_bytes([gain_freq[0], gain_freq[1]]) as f64) / 10.0;
    let freq = u16::from_le_bytes([gain_freq[2], gain_freq[3]]) as f64;
    let q = (u16::from_le_bytes([q_type[0], q_type[1]]) as f64) / 1000.0;
    FilterBand::new(ftype, freq, gain, q).clamped()
}

// --- 2. Moondrop 8-band & WalkPlay (96 kHz Q2.30 biquad + metadata) ---

pub fn md8_type_to_code(ft: FilterType) -> u8 {
    match ft {
        FilterType::LowShelf => 1,
        FilterType::Peaking => 2,
        FilterType::HighShelf => 3,
    }
}

pub fn md8_code_to_type(code: u8) -> FilterType {
    match code {
        1 => FilterType::LowShelf,
        3 => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

pub fn md8_encode_biquad(band: &FilterBand) -> [i32; 5] {
    let b = band.clamped();
    let a = 10.0_f64.powf(b.gain / 40.0);
    let w0 = 2.0 * PI * b.frequency / 96_000.0;
    let sin_w0 = w0.sin();
    let cos_w0 = w0.cos();
    let alpha = sin_w0 / (2.0 * b.q);

    let (b0, b1, b2, a0, a1, a2) = match b.filter_type {
        FilterType::LowShelf | FilterType::HighShelf => {
            let term = ((a + 1.0 / a) * (1.0 / b.q - 1.0) + 2.0).max(1e-9);
            let shelf_alpha = (sin_w0 / 2.0) * term.sqrt();
            let s = 2.0 * a.sqrt() * shelf_alpha;
            if b.filter_type == FilterType::LowShelf {
                (
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 - s),
                    (a + 1.0) + (a - 1.0) * cos_w0 + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
                    (a + 1.0) + (a - 1.0) * cos_w0 - s,
                )
            } else {
                (
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 - s),
                    (a + 1.0) - (a - 1.0) * cos_w0 + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
                    (a + 1.0) - (a - 1.0) * cos_w0 - s,
                )
            }
        }
        FilterType::Peaking => (
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        ),
    };

    let nb0 = b0 / a0;
    let nb1 = b1 / a0;
    let nb2 = b2 / a0;
    let na1 = -(a1 / a0);
    let na2 = -(a2 / a0);
    let scale = 1_073_741_824.0; // Q2.30 fixed point
    [
        (nb0 * scale).round() as i32,
        (nb1 * scale).round() as i32,
        (nb2 * scale).round() as i32,
        (na1 * scale).round() as i32,
        (na2 * scale).round() as i32,
    ]
}

pub fn md8_build_write_packet_with_slot(index: u8, band: &FilterBand, slot: u8) -> [u8; 64] {
    let b = band.clamped();
    let mut pkt = [0u8; 64];
    pkt[0] = MD_REPORT_ID;
    pkt[1] = MD_CMD_WRITE;
    pkt[2] = MD_CMD_UPDATE_EQ;
    pkt[3] = 0x18;
    pkt[4] = 0x00;
    pkt[5] = index;
    pkt[6] = 0x00;
    pkt[7] = 0x00;

    let coeffs = md8_encode_biquad(&b);
    for (k, val) in coeffs.iter().enumerate() {
        pkt[8 + k * 4..12 + k * 4].copy_from_slice(&val.to_le_bytes());
    }

    let freq_u16 = b.frequency.round() as u16;
    let q_u16 = (b.q * 256.0).round() as u16;
    let gain_i16 = (b.gain * 256.0).round() as i16;
    pkt[28..30].copy_from_slice(&freq_u16.to_le_bytes());
    pkt[30..32].copy_from_slice(&q_u16.to_le_bytes());
    pkt[32..34].copy_from_slice(&gain_i16.to_le_bytes());
    pkt[34] = md8_type_to_code(b.filter_type);
    pkt[35] = 0;
    pkt[36] = slot;
    pkt
}

pub fn md8_build_write_packet(index: u8, band: &FilterBand) -> [u8; 64] {
    md8_build_write_packet_with_slot(index, band, 7)
}

// --- 3. FiiO / JadeAudio / SNOWSKY (big-endian AA 0A / BB 0B ... EE) ---

pub fn fiio_type_to_code(ft: FilterType) -> u8 {
    match ft {
        FilterType::Peaking => 0,
        FilterType::LowShelf => 1,
        FilterType::HighShelf => 2,
    }
}

pub fn fiio_code_to_type(code: u8) -> FilterType {
    match code {
        1 => FilterType::LowShelf,
        2 => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

pub fn fiio_build_set_filter_packet(report_id: u8, index: u8, band: &FilterBand) -> [u8; 17] {
    let b = band.clamped();
    let gain_be = ((b.gain * 10.0).round() as i16).to_be_bytes();
    let freq_be = (b.frequency.round() as u16).to_be_bytes();
    let q_be = ((b.q * 100.0).round() as u16).to_be_bytes();
    [
        report_id,
        FIIO_SET_H1,
        FIIO_SET_H2,
        0x00,
        0x00,
        FIIO_CMD_FILTER_PARAMS,
        0x08,
        index,
        gain_be[0],
        gain_be[1],
        freq_be[0],
        freq_be[1],
        q_be[0],
        q_be[1],
        fiio_type_to_code(b.filter_type),
        0x00,
        FIIO_END,
    ]
}

pub fn fiio_decode_filter_payload(payload: &[u8]) -> Option<(usize, FilterBand)> {
    // payload starts at FIIO_GET_H1 (0xBB), i.e. after optional report_id byte
    if payload.len() < 14
        || payload[0] != FIIO_GET_H1
        || payload[1] != FIIO_GET_H2
        || payload[4] != FIIO_CMD_FILTER_PARAMS
    {
        return None;
    }
    let idx = payload[6] as usize;
    let gain = (i16::from_be_bytes([payload[7], payload[8]]) as f64) / 10.0;
    let freq = u16::from_be_bytes([payload[9], payload[10]]) as f64;
    let q_raw = u16::from_be_bytes([payload[11], payload[12]]) as f64 / 100.0;
    let q = if q_raw < 0.05 { 1.0 } else { q_raw };
    let ft = fiio_code_to_type(payload[13]);
    let band = if (10.0..=24_000.0).contains(&freq) {
        FilterBand::new(ft, freq, gain, q).clamped()
    } else {
        FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0)
    };
    Some((idx, band))
}

pub fn fiio_build_global_gain_packet(report_id: u8, preamp_db: f64) -> [u8; 11] {
    let g_be = ((preamp_db.clamp(-12.0, 12.0) * 10.0).round() as i16).to_be_bytes();
    [
        report_id,
        FIIO_SET_H1,
        FIIO_SET_H2,
        0x00,
        0x00,
        FIIO_CMD_GLOBAL_GAIN,
        0x02,
        g_be[0],
        g_be[1],
        0x00,
        FIIO_END,
    ]
}

pub fn fiio_build_simple_cmd(report_id: u8, is_set: bool, cmd: u8, arg: Option<u8>) -> Vec<u8> {
    let (h1, h2) = if is_set {
        (FIIO_SET_H1, FIIO_SET_H2)
    } else {
        (FIIO_GET_H1, FIIO_GET_H2)
    };
    match arg {
        Some(v) => vec![report_id, h1, h2, 0x00, 0x00, cmd, 0x01, v, 0x00, FIIO_END],
        None => vec![report_id, h1, h2, 0x00, 0x00, cmd, 0x00, 0x00, FIIO_END],
    }
}

// ---------------------------------------------------------------------------
// Abstract Low-level HID Channel & Shared Protocol State Machine
// ---------------------------------------------------------------------------

trait HidChannel {
    fn drain_input(&mut self);
    fn write_report(&mut self, data: &[u8]) -> Result<(), String>;
    fn read_report(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, String>;
}

fn kt_command(
    ch: &mut dyn HidChannel,
    reg: u8,
    cmd: u8,
    value: [u8; 4],
    ignore_disconnect: bool,
) -> Result<[u8; 4], String> {
    let pkt = kt_packet(reg, cmd, value);
    for _attempt in 0..2 {
        ch.drain_input();
        ch.write_report(&pkt)?;

        let mut buf = [0u8; 64];
        for _ in 0..8 {
            match ch.read_report(&mut buf, 450) {
                Ok(0) => break,
                Ok(n) => {
                    if n >= 11 && buf[0] == KT_REPORT_ID && buf[1] == reg && buf[5] == cmd {
                        return Ok([buf[7], buf[8], buf[9], buf[10]]);
                    }
                }
                Err(e) => {
                    if ignore_disconnect {
                        return Ok([0; 4]);
                    }
                    return Err(e);
                }
            }
        }
        if ignore_disconnect {
            return Ok([0; 4]);
        }
    }
    Err(format!(
        "Sin respuesta del DSP para registro 0x{reg:02X} (cmd 0x{cmd:02X})."
    ))
}

fn md8_send_and_read(
    ch: &mut dyn HidChannel,
    payload_with_id: &[u8],
    expect_cmd0: u8,
    expect_cmd1: u8,
) -> Result<Vec<u8>, String> {
    ch.drain_input();
    ch.write_report(payload_with_id)?;

    let mut buf = [0u8; 64];
    for _ in 0..8 {
        let n = ch.read_report(&mut buf, 600)?;
        if n == 0 {
            break;
        }
        let reply = &buf[..n];
        let offset = if reply.len() > 1 && reply[0] == MD_REPORT_ID {
            1
        } else {
            0
        };
        if reply.len() >= offset + 2 && reply[offset] == expect_cmd0 && reply[offset + 1] == expect_cmd1 {
            return Ok(reply[offset..].to_vec());
        }
    }
    Err(format!(
        "Sin respuesta del DSP (cmd 0x{expect_cmd1:02X})."
    ))
}

fn fiio_send_and_read(
    ch: &mut dyn HidChannel,
    req: &[u8],
    report_id: u8,
    expect_cmd: u8,
) -> Result<Vec<u8>, String> {
    ch.drain_input();
    ch.write_report(req)?;

    let mut buf = [0u8; 64];
    for _ in 0..8 {
        let n = ch.read_report(&mut buf, 600)?;
        if n == 0 {
            break;
        }
        let reply = &buf[..n];
        let offset = if reply.len() > 2 && reply[0] == report_id && reply[1] == FIIO_GET_H1 {
            1
        } else {
            0
        };
        if reply.len() >= offset + 7
            && reply[offset] == FIIO_GET_H1
            && reply[offset + 1] == FIIO_GET_H2
            && reply[offset + 4] == expect_cmd
        {
            return Ok(reply[offset..].to_vec());
        }
    }
    Err(format!(
        "Sin respuesta del DSP FiiO (cmd 0x{expect_cmd:02X})."
    ))
}

fn read_eq_on_channel(
    ch: &mut dyn HidChannel,
    info: &MoondropDeviceInfo,
) -> Result<(bool, Vec<FilterBand>), String> {
    match info.protocol {
        HidProtocol::KtMicro => {
            let slot = kt_command(ch, KT_REG_SLOT, KT_READ, [KT_SLOT_ON, 0, 0, 0], false)?[0];
            let mut bands = Vec::with_capacity(info.max_bands);
            for i in 0..info.max_bands {
                let reg = KT_REG_BAND0 + (2 * i as u8);
                let gf = kt_command(ch, reg, KT_READ, [0; 4], false)?;
                let qt = kt_command(ch, reg + 1, KT_READ, [0; 4], false)?;
                bands.push(kt_decode_band(gf, qt));
            }
            Ok((slot != KT_SLOT_OFF, bands))
        }
        HidProtocol::Moondrop8 | HidProtocol::WalkPlay => {
            let mut bands = Vec::with_capacity(info.max_bands);
            for i in 0..info.max_bands {
                let req = [
                    info.report_id,
                    MD_CMD_READ,
                    MD_CMD_UPDATE_EQ,
                    0x18,
                    0x00,
                    i as u8,
                    0x00,
                ];
                let data = md8_send_and_read(ch, &req, MD_CMD_READ, MD_CMD_UPDATE_EQ)?;
                if data.len() >= 34 {
                    let freq = u16::from_le_bytes([data[27], data[28]]) as f64;
                    let q = (u16::from_le_bytes([data[29], data[30]]) as f64) / 256.0;
                    let gain = (i16::from_le_bytes([data[31], data[32]]) as f64) / 256.0;
                    let ftype = md8_code_to_type(data[33]);
                    if (10.0..24_000.0).contains(&freq) {
                        bands.push(FilterBand::new(ftype, freq, gain, q).clamped());
                    } else {
                        bands.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
                    }
                } else {
                    bands.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
                }
            }
            Ok((true, bands))
        }
        HidProtocol::Fiio => {
            let preset_req = fiio_build_simple_cmd(info.report_id, false, FIIO_CMD_PRESET_SWITCH, None);
            let enabled = fiio_send_and_read(ch, &preset_req, info.report_id, FIIO_CMD_PRESET_SWITCH)
                .map(|p| p.get(6).copied().unwrap_or(info.user_slot) != info.bypass_slot)
                .unwrap_or(true);

            let mut bands = vec![FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0); info.max_bands];
            for i in 0..info.max_bands {
                let req = fiio_build_simple_cmd(
                    info.report_id,
                    false,
                    FIIO_CMD_FILTER_PARAMS,
                    Some(i as u8),
                );
                if let Ok(payload) = fiio_send_and_read(ch, &req, info.report_id, FIIO_CMD_FILTER_PARAMS) {
                    if let Some((idx, band)) = fiio_decode_filter_payload(&payload) {
                        if idx < bands.len() {
                            bands[idx] = band;
                        }
                    }
                }
            }
            Ok((enabled, bands))
        }
    }
}

fn write_eq_on_channel(
    ch: &mut dyn HidChannel,
    info: &MoondropDeviceInfo,
    bands: &[FilterBand],
    commit: bool,
    enable_eq: bool,
) -> Result<(), String> {
    let n = info.max_bands;
    let mut fitted = Vec::with_capacity(n);
    for b in bands.iter().take(n) {
        let mut bc = b.clamped_with_max_gain(info.max_gain_db);
        if !info.supports_shelves && bc.filter_type != FilterType::Peaking {
            bc.filter_type = FilterType::Peaking;
        }
        fitted.push(bc);
    }
    while fitted.len() < n {
        fitted.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
    }

    match info.protocol {
        HidProtocol::KtMicro => {
            let slot = kt_command(ch, KT_REG_SLOT, KT_READ, [KT_SLOT_ON, 0, 0, 0], false)?[0];
            if enable_eq && slot == KT_SLOT_OFF {
                kt_command(ch, KT_REG_SLOT, KT_WRITE, [KT_SLOT_ON, 0, 0, 0], false)?;
            } else if !enable_eq && slot != KT_SLOT_OFF {
                kt_command(ch, KT_REG_SLOT, KT_WRITE, [KT_SLOT_OFF, 0, 0, 0], false)?;
            }

            for (i, band) in fitted.iter().enumerate() {
                let reg = KT_REG_BAND0 + (2 * i as u8);
                let (gf, qt) = kt_encode_band(band);
                kt_command(ch, reg, KT_WRITE, gf, false)?;
                kt_command(ch, reg + 1, KT_WRITE, qt, false)?;
                let rgf = kt_command(ch, reg, KT_READ, [0; 4], false)?;
                let rqt = kt_command(ch, reg + 1, KT_READ, [0; 4], false)?;
                if rgf != gf || rqt != qt {
                    return Err(format!(
                        "Verificación fallida en banda {}: el DSP devolvió datos distintos. No se guardó en memoria flash.",
                        i + 1
                    ));
                }
            }

            if commit {
                let _ = kt_command(ch, 0x00, KT_COMMIT, [0; 4], true);
            }
            Ok(())
        }
        HidProtocol::Moondrop8 => {
            for (i, band) in fitted.iter().enumerate() {
                let b = if enable_eq {
                    band.clone()
                } else {
                    FilterBand::new(FilterType::Peaking, band.frequency, 0.0, 1.0)
                };
                let pkt = md8_build_write_packet_with_slot(i as u8, &b, info.user_slot);
                ch.write_report(&pkt)?;

                let mut enable_pkt = [0u8; 64];
                enable_pkt[0] = info.report_id;
                enable_pkt[1] = MD_CMD_WRITE;
                enable_pkt[2] = MD_CMD_APPLY_COEFF;
                enable_pkt[3] = i as u8;
                enable_pkt[4] = 0;
                enable_pkt[5] = 255;
                enable_pkt[6] = 255;
                enable_pkt[7] = 255;
                ch.write_report(&enable_pkt)?;
                thread::sleep(Duration::from_millis(15));
            }
            if commit {
                ch.write_report(&[info.report_id, MD_CMD_WRITE, MD_CMD_SAVE_FLASH])?;
            }
            Ok(())
        }
        HidProtocol::WalkPlay => {
            for (i, band) in fitted.iter().enumerate() {
                let b = if enable_eq {
                    band.clone()
                } else {
                    FilterBand::new(FilterType::Peaking, band.frequency, 0.0, 1.0)
                };
                let pkt = md8_build_write_packet_with_slot(i as u8, &b, info.user_slot);
                ch.write_report(&pkt)?;
                thread::sleep(Duration::from_millis(18));
            }
            thread::sleep(Duration::from_millis(60));
            // Write global preamp (CMD 0x03, 1-byte signed dB)
            let preamp = if enable_eq {
                compute_safe_preamp_db(&fitted).round().clamp(-12.0, 0.0) as i8
            } else {
                0i8
            };
            ch.write_report(&[info.report_id, MD_CMD_WRITE, WP_CMD_GLOBAL_GAIN, 0x01, preamp as u8, 0x00])?;
            thread::sleep(Duration::from_millis(25));

            // WalkPlay commit sequence (matches walkplayHidHandler.js)
            ch.write_report(&[info.report_id, MD_CMD_WRITE, 0x05, 0x00])?;
            thread::sleep(Duration::from_millis(15));
            ch.write_report(&[info.report_id, MD_CMD_WRITE, 0x17, 0x00])?;
            thread::sleep(Duration::from_millis(15));
            ch.write_report(&[
                info.report_id,
                MD_CMD_WRITE,
                MD_CMD_APPLY_COEFF,
                0x04,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0x00,
            ])?;
            if commit {
                thread::sleep(Duration::from_millis(40));
                ch.write_report(&[info.report_id, MD_CMD_WRITE, MD_CMD_SAVE_FLASH, 0x01, 0x00])?;
            }
            Ok(())
        }
        HidProtocol::Fiio => {
            let preamp = if enable_eq {
                compute_safe_preamp_db(&fitted)
            } else {
                0.0
            };
            let g_pkt = fiio_build_global_gain_packet(info.report_id, preamp);
            ch.write_report(&g_pkt)?;

            let cnt_pkt = fiio_build_simple_cmd(
                info.report_id,
                true,
                FIIO_CMD_FILTER_COUNT,
                Some(fitted.len() as u8),
            );
            ch.write_report(&cnt_pkt)?;
            thread::sleep(Duration::from_millis(60));

            for (i, band) in fitted.iter().enumerate() {
                let b = if enable_eq {
                    band.clone()
                } else {
                    FilterBand::new(FilterType::Peaking, band.frequency, 0.0, 1.0)
                };
                let fpkt = fiio_build_set_filter_packet(info.report_id, i as u8, &b);
                ch.write_report(&fpkt)?;
                thread::sleep(Duration::from_millis(10));
            }
            thread::sleep(Duration::from_millis(60));

            let target_slot = if enable_eq {
                info.user_slot
            } else {
                info.bypass_slot
            };
            let switch_pkt = fiio_build_simple_cmd(
                info.report_id,
                true,
                FIIO_CMD_PRESET_SWITCH,
                Some(target_slot),
            );
            ch.write_report(&switch_pkt)?;

            if commit {
                let save_cmd = if info.fiio_save_v2 {
                    FIIO_CMD_SAVE_V2
                } else {
                    FIIO_CMD_SAVE_TO_DEVICE
                };
                let save_pkt = fiio_build_simple_cmd(info.report_id, true, save_cmd, Some(info.user_slot));
                ch.write_report(&save_pkt)?;
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Linux Direct /dev/hidraw Implementation (Zero external C library deps)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::ffi::CString;
    use std::fs;
    use std::path::Path;

    pub fn scan_all_hid_devices() -> Vec<RawHidDevice> {
        let mut entries: Vec<String> = fs::read_dir("/sys/class/hidraw")
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with("hidraw"))
            .collect();
        entries.sort();

        let mut out = Vec::new();
        for hidraw_name in entries {
            let uevent_path = format!("/sys/class/hidraw/{hidraw_name}/device/uevent");
            let Ok(content) = fs::read_to_string(&uevent_path) else {
                continue;
            };
            let mut vid: u16 = 0;
            let mut pid: u16 = 0;
            let mut hid_name = String::new();

            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("HID_ID=") {
                    let parts: Vec<&str> = rest.trim().split(':').collect();
                    if parts.len() == 3 {
                        vid = u16::from_str_radix(parts[1], 16).unwrap_or(0);
                        pid = u16::from_str_radix(parts[2], 16).unwrap_or(0);
                    }
                } else if let Some(rest) = line.strip_prefix("HID_NAME=") {
                    hid_name = rest.trim().to_string();
                }
            }
            let dev_path = format!("/dev/{hidraw_name}");
            if !Path::new(&dev_path).exists() {
                continue;
            }
            let matched = classify_hid_device(&dev_path, vid, pid, &hid_name);
            out.push(RawHidDevice {
                path: dev_path,
                vid,
                pid,
                vid_pid: format!("{vid:04x}:{pid:04x}"),
                hid_name,
                recognized: matched.is_some(),
                matched_name: matched.as_ref().map(|m| m.display_name.clone()),
                protocol: matched.as_ref().map(|m| m.protocol),
            });
        }
        out
    }

    pub fn scan_moondrop_devices() -> Vec<MoondropDeviceInfo> {
        let mut found = Vec::new();
        for raw in scan_all_hid_devices() {
            if let Some(info) = classify_hid_device(&raw.path, raw.vid, raw.pid, &raw.hid_name) {
                found.push(info);
            }
        }
        found
    }

    /// Ultra-fast (<0.5 ms) hardware presence fingerprint replacing `moondrop_poll.c`.
    pub fn poll_fingerprint() -> String {
        let devs = scan_moondrop_devices();
        let usb_part: Vec<String> = devs
            .iter()
            .map(|d| {
                let name = d.hidraw_path.trim_start_matches("/dev/");
                format!("{name}:{:08x}:{:08x}", d.vid as u32, d.pid as u32)
            })
            .collect();

        let mut bt_links: Vec<String> = fs::read_dir("/sys/class/bluetooth")
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with("hci") && n.contains(':'))
            .collect();
        bt_links.sort();

        format!("usb={}|bt={}", usb_part.join(","), bt_links.join(","))
    }

    pub struct LinuxHidrawChannel {
        fd: libc::c_int,
    }

    impl LinuxHidrawChannel {
        pub fn open(info: &MoondropDeviceInfo) -> Result<Self, String> {
            let cpath = CString::new(info.hidraw_path.as_str()).map_err(|e| e.to_string())?;
            let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    return Err(format!(
                        "Sin permisos para abrir {} ({}, {}). Instala las reglas udev desde AVANZADO.",
                        info.hidraw_path, info.display_name, info.vid_pid()
                    ));
                }
                return Err(format!("Error abriendo {}: {err}", info.hidraw_path));
            }
            Ok(Self { fd })
        }

        fn wait_readable(&self, timeout_ms: i32) -> bool {
            let mut pfd = libc::pollfd {
                fd: self.fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            rc > 0 && (pfd.revents & libc::POLLIN) != 0
        }
    }

    impl Drop for LinuxHidrawChannel {
        fn drop(&mut self) {
            if self.fd >= 0 {
                unsafe {
                    libc::close(self.fd);
                }
                self.fd = -1;
            }
        }
    }

    impl HidChannel for LinuxHidrawChannel {
        fn drain_input(&mut self) {
            let mut buf = [0u8; 64];
            while self.wait_readable(0) {
                let n = unsafe {
                    libc::read(
                        self.fd,
                        buf.as_mut_ptr() as *mut libc::c_void,
                        buf.len(),
                    )
                };
                if n <= 0 {
                    break;
                }
            }
        }

        fn write_report(&mut self, data: &[u8]) -> Result<(), String> {
            let n = unsafe {
                libc::write(
                    self.fd,
                    data.as_ptr() as *const libc::c_void,
                    data.len(),
                )
            };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("Error escribiendo comando HID: {err}"));
            }
            Ok(())
        }

        fn read_report(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, String> {
            if !self.wait_readable(timeout_ms as i32) {
                return Ok(0);
            }
            let n = unsafe {
                libc::read(
                    self.fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("Error leyendo respuesta HID: {err}"));
            }
            Ok(n as usize)
        }
    }

    pub fn open_channel(info: &MoondropDeviceInfo) -> Result<Box<dyn HidChannel>, String> {
        Ok(Box::new(LinuxHidrawChannel::open(info)?))
    }
}

// ---------------------------------------------------------------------------
// Windows / Non-Linux `hidapi` Implementation
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::*;
    use std::collections::HashSet;
    use std::ffi::CString;

    pub fn scan_all_hid_devices() -> Vec<RawHidDevice> {
        let Ok(api) = hidapi::HidApi::new() else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for dev in api.device_list() {
            let path_str = dev.path().to_string_lossy().into_owned();
            if path_str.is_empty() || !seen.insert(path_str.clone()) {
                continue;
            }
            let prod = dev.product_string().unwrap_or("").to_string();
            let vid = dev.vendor_id();
            let pid = dev.product_id();
            let matched = classify_hid_device(&path_str, vid, pid, &prod);
            out.push(RawHidDevice {
                path: path_str,
                vid,
                pid,
                vid_pid: format!("{vid:04x}:{pid:04x}"),
                hid_name: prod,
                recognized: matched.is_some(),
                matched_name: matched.as_ref().map(|m| m.display_name.clone()),
                protocol: matched.as_ref().map(|m| m.protocol),
            });
        }
        out
    }

    pub fn scan_moondrop_devices() -> Vec<MoondropDeviceInfo> {
        let mut found = Vec::new();
        for raw in scan_all_hid_devices() {
            if let Some(info) = classify_hid_device(&raw.path, raw.vid, raw.pid, &raw.hid_name) {
                found.push(info);
            }
        }
        found
    }

    pub fn poll_fingerprint() -> String {
        let devs = scan_moondrop_devices();
        let usb_part: Vec<String> = devs
            .iter()
            .map(|d| format!("{:04x}:{:04x}", d.vid, d.pid))
            .collect();
        format!("usb={}|bt=", usb_part.join(","))
    }

    struct HidApiChannel {
        dev: hidapi::HidDevice,
    }

    impl HidChannel for HidApiChannel {
        fn drain_input(&mut self) {
            let mut buf = [0u8; 64];
            while let Ok(n) = self.dev.read_timeout(&mut buf, 0) {
                if n == 0 {
                    break;
                }
            }
        }

        fn write_report(&mut self, data: &[u8]) -> Result<(), String> {
            self.dev
                .write(data)
                .map(|_| ())
                .map_err(|e| format!("Error escribiendo comando HID: {e}"))
        }

        fn read_report(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, String> {
            self.dev
                .read_timeout(buf, timeout_ms as i32)
                .map_err(|e| format!("Error leyendo respuesta HID: {e}"))
        }
    }

    pub fn open_channel(info: &MoondropDeviceInfo) -> Result<Box<dyn HidChannel>, String> {
        let api = hidapi::HidApi::new().map_err(|e| format!("Error inicializando hidapi: {e}"))?;
        let cpath = CString::new(info.hidraw_path.as_str()).map_err(|e| e.to_string())?;
        let dev = api
            .open_path(cpath.as_c_str())
            .map_err(|e| format!("No se pudo abrir el dispositivo HID ({}): {e}", info.display_name))?;
        Ok(Box::new(HidApiChannel { dev }))
    }
}

pub use imp::{poll_fingerprint, scan_all_hid_devices, scan_moondrop_devices};

pub fn find_active_moondrop_device() -> Option<MoondropDeviceInfo> {
    scan_moondrop_devices().into_iter().next()
}

pub fn read_device_eq(
    info: Option<&MoondropDeviceInfo>,
) -> Result<(MoondropDeviceInfo, bool, Vec<FilterBand>), String> {
    let owned_info = match info {
        Some(i) => i.clone(),
        None => find_active_moondrop_device()
            .ok_or_else(|| "No se detectó ningún dispositivo DSP USB-C compatible.".to_string())?,
    };
    let mut ch = imp::open_channel(&owned_info)?;
    let (enabled, bands) = read_eq_on_channel(ch.as_mut(), &owned_info)?;
    Ok((owned_info, enabled, bands))
}

pub fn write_device_eq(
    bands: &[FilterBand],
    commit: bool,
    enable_eq: bool,
    info: Option<&MoondropDeviceInfo>,
) -> Result<MoondropDeviceInfo, String> {
    let owned_info = match info {
        Some(i) => i.clone(),
        None => find_active_moondrop_device()
            .ok_or_else(|| "No se detectó ningún dispositivo DSP USB-C compatible.".to_string())?,
    };
    {
        let mut ch = imp::open_channel(&owned_info)?;
        write_eq_on_channel(ch.as_mut(), &owned_info, bands, commit, enable_eq)?;
    }

    if commit && owned_info.disconnects_on_commit {
        let deadline = Instant::now() + Duration::from_millis(4500);
        thread::sleep(Duration::from_millis(800));
        while Instant::now() < deadline {
            if find_active_moondrop_device().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }
    }
    Ok(owned_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ktmicro_band_roundtrip_matches_python_spec() {
        let band = FilterBand::new(FilterType::LowShelf, 105.0, -3.4, 0.707);
        let (gf, qt) = kt_encode_band(&band);
        assert_eq!(gf, [0xDE, 0xFF, 0x69, 0x00]);
        assert_eq!(qt, [0xC3, 0x02, 3, 0]);

        let decoded = kt_decode_band(gf, qt);
        assert_eq!(decoded.filter_type, FilterType::LowShelf);
        assert_eq!(decoded.frequency, 105.0);
        assert!((decoded.gain - (-3.4)).abs() < 1e-6);
        assert!((decoded.q - 0.707).abs() < 1e-6);
    }

    #[test]
    fn moondrop8_packet_header_and_metadata_layout() {
        let band = FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0);
        let pkt = md8_build_write_packet(2, &band);
        assert_eq!(pkt[0], 0x4B);
        assert_eq!(pkt[1], 0x01);
        assert_eq!(pkt[2], 0x09);
        assert_eq!(pkt[3], 0x18);
        assert_eq!(pkt[5], 2);
        assert_eq!(&pkt[8..12], &1_073_741_824_i32.to_le_bytes());
        assert_eq!(u16::from_le_bytes([pkt[28], pkt[29]]), 1000);
        assert_eq!(u16::from_le_bytes([pkt[30], pkt[31]]), 256);
        assert_eq!(i16::from_le_bytes([pkt[32], pkt[33]]), 0);
        assert_eq!(pkt[34], 2);
        assert_eq!(pkt[36], 7);
    }

    #[test]
    fn fiio_filter_packet_big_endian_roundtrip() {
        let band = FilterBand::new(FilterType::HighShelf, 8500.0, -4.5, 1.41);
        let pkt = fiio_build_set_filter_packet(7, 3, &band);
        assert_eq!(pkt[0], 7);
        assert_eq!(pkt[1], 0xAA);
        assert_eq!(pkt[2], 0x0A);
        assert_eq!(pkt[5], 0x15);
        assert_eq!(pkt[6], 8);
        assert_eq!(pkt[7], 3);
        assert_eq!(pkt[16], 0xEE);

        // Simulate GET response (BB 0B instead of AA 0A)
        let mut resp = pkt[1..].to_vec();
        resp[0] = FIIO_GET_H1;
        resp[1] = FIIO_GET_H2;
        let (idx, dec) = fiio_decode_filter_payload(&resp).unwrap();
        assert_eq!(idx, 3);
        assert_eq!(dec.filter_type, FilterType::HighShelf);
        assert_eq!(dec.frequency, 8500.0);
        assert!((dec.gain - (-4.5)).abs() < 1e-6);
        assert!((dec.q - 1.41).abs() < 1e-6);
    }

    #[test]
    fn classifies_multiprotocol_dsp_families() {
        let c = classify_hid_device("/dev/hidraw4", 0x31B2, 0x0113, "MOONDROP CHU2 DSP").unwrap();
        assert_eq!(c.protocol, HidProtocol::KtMicro);
        assert_eq!(c.max_bands, 5);

        let k = classify_hid_device("/dev/hidraw5", 0x31B2, 0x0200, "Kiwi Ears Allegro Mini").unwrap();
        assert_eq!(k.protocol, HidProtocol::KtMicro);

        let f = classify_hid_device("/dev/hidraw6", 0x2FC6, 0xF001, "MOONDROP FreeDSP Pro").unwrap();
        assert_eq!(f.protocol, HidProtocol::Moondrop8);
        assert_eq!(f.max_bands, 8);

        let ja11 = classify_hid_device("/dev/hidraw7", 0x2972, 0x0011, "JadeAudio JA11").unwrap();
        assert_eq!(ja11.protocol, HidProtocol::Fiio);
        assert_eq!(ja11.max_bands, 5);
        assert_eq!(ja11.report_id, 2);

        let ka17 = classify_hid_device("/dev/hidraw8", 0x2972, 0x0017, "FIIO KA17").unwrap();
        assert_eq!(ka17.protocol, HidProtocol::Fiio);
        assert_eq!(ka17.max_bands, 10);
        assert_eq!(ka17.report_id, 1);

        let pmax = classify_hid_device("/dev/hidraw9", 0x3302, 0x43CC, "CrinEar Protocol Max").unwrap();
        assert_eq!(pmax.protocol, HidProtocol::WalkPlay);
        assert_eq!(pmax.max_bands, 10);
    }
}
