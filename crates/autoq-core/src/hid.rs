//! Unified cross-platform USB-C HID protocol & transport for Moondrop DSP devices:
//! 1. KTMicro 5-band devices (`VID 0x31B2`): Chu 2 DSP (`0x0113`), CDSP, Quark 2, Jiu, May.
//! 2. Moondrop 8-band devices: Rays, AG Rays, Marigold, FreeDSP Pro/Mini, DAWN PRO 2,
//!    Echo A, MOONRIVER 3, DHA15.

use crate::biquad::{FilterBand, FilterType};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::thread;
use std::time::{Duration, Instant};

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

// Moondrop 8-band constants
pub const MD_REPORT_ID: u8 = 0x4B;
pub const MD_CMD_WRITE: u8 = 0x01;
pub const MD_CMD_READ: u8 = 0x80;
pub const MD_CMD_UPDATE_EQ: u8 = 0x09;
pub const MD_CMD_APPLY_COEFF: u8 = 0x0A;
pub const MD_CMD_SAVE_FLASH: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HidProtocol {
    #[serde(rename = "ktmicro")]
    KtMicro,
    #[serde(rename = "moondrop8")]
    Moondrop8,
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
}

impl MoondropDeviceInfo {
    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", self.vid, self.pid)
    }
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

pub fn classify_hid_device(
    dev_path: &str,
    vid: u16,
    pid: u16,
    hid_name: &str,
) -> Option<MoondropDeviceInfo> {
    let low = hid_name.to_ascii_lowercase();

    if vid == KT_VID {
        let (display_name, hint) = if pid == CHU2_DSP_PID || low.contains("chu2") || low.contains("chu 2") {
            ("Moondrop Chu 2 DSP".to_string(), "Moondrop Chu 2".to_string())
        } else if low.contains("quark") {
            ("Moondrop Quark 2 DSP".to_string(), "Moondrop".to_string())
        } else if low.contains("jiu") {
            ("Moondrop Jiu DSP".to_string(), "Moondrop Jiu".to_string())
        } else if low.contains("may") {
            ("Moondrop May DSP".to_string(), "Moondrop May".to_string())
        } else if low.contains("cdsp") || low.contains("moondrop") || cfg!(target_os = "windows") {
            let disp = if hid_name.trim().is_empty() {
                format!("Moondrop CDSP ({vid:04x}:{pid:04x})")
            } else if hid_name.to_ascii_lowercase().starts_with("moondrop") {
                hid_name.trim().to_string()
            } else {
                format!("Moondrop {}", hid_name.trim())
            };
            (disp, "Moondrop".to_string())
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
        });
    }

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
            });
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Binary Protocol Codecs (KTMicro 5-band & Moondrop 8-band)
// ---------------------------------------------------------------------------

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

pub fn md8_build_write_packet(index: u8, band: &FilterBand) -> [u8; 64] {
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
    pkt[36] = 7;
    pkt
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
        "Sin respuesta del DSP Moondrop (cmd 0x{expect_cmd1:02X})."
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
        HidProtocol::Moondrop8 => {
            let mut bands = Vec::with_capacity(info.max_bands);
            for i in 0..info.max_bands {
                let req = [
                    MD_REPORT_ID,
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
                let pkt = md8_build_write_packet(i as u8, band);
                ch.write_report(&pkt)?;

                let mut enable_pkt = [0u8; 64];
                enable_pkt[0] = MD_REPORT_ID;
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
                ch.write_report(&[MD_REPORT_ID, MD_CMD_WRITE, MD_CMD_SAVE_FLASH])?;
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

    pub fn scan_moondrop_devices() -> Vec<MoondropDeviceInfo> {
        let mut entries: Vec<String> = fs::read_dir("/sys/class/hidraw")
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with("hidraw"))
            .collect();
        entries.sort();

        let mut found = Vec::new();
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
            if let Some(info) = classify_hid_device(&dev_path, vid, pid, &hid_name) {
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
                        "Sin permisos para abrir {} ({}, {}). Agrega una regla udev para idVendor={:04x}, idProduct={:04x}.",
                        info.hidraw_path, info.display_name, info.vid_pid(), info.vid, info.pid
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

    pub fn scan_moondrop_devices() -> Vec<MoondropDeviceInfo> {
        let Ok(api) = hidapi::HidApi::new() else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for dev in api.device_list() {
            let path_str = dev.path().to_string_lossy().into_owned();
            if path_str.is_empty() || !seen.insert(path_str.clone()) {
                continue;
            }
            let prod = dev.product_string().unwrap_or("");
            if let Some(info) = classify_hid_device(&path_str, dev.vendor_id(), dev.product_id(), prod) {
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

pub use imp::{poll_fingerprint, scan_moondrop_devices};

pub fn find_active_moondrop_device() -> Option<MoondropDeviceInfo> {
    scan_moondrop_devices().into_iter().next()
}

pub fn read_device_eq(
    info: Option<&MoondropDeviceInfo>,
) -> Result<(MoondropDeviceInfo, bool, Vec<FilterBand>), String> {
    let owned_info = match info {
        Some(i) => i.clone(),
        None => find_active_moondrop_device()
            .ok_or_else(|| "No se detectó ningún dispositivo Moondrop DSP por USB-C.".to_string())?,
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
            .ok_or_else(|| "No se detectó ningún dispositivo Moondrop DSP por USB-C.".to_string())?,
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
        // -34 in i16 LE = 0xFFDE = [0xDE, 0xFF], 105 in u16 LE = [0x69, 0x00]
        assert_eq!(gf, [0xDE, 0xFF, 0x69, 0x00]);
        // 707 in u16 LE = [0xC3, 0x02], low_shelf = 3
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
        // At 0 dB peaking, b0/a0 == 1.0 -> Q2.30 == 1073741824 (0x40000000)
        assert_eq!(&pkt[8..12], &1_073_741_824_i32.to_le_bytes());
        assert_eq!(u16::from_le_bytes([pkt[28], pkt[29]]), 1000);
        assert_eq!(u16::from_le_bytes([pkt[30], pkt[31]]), 256);
        assert_eq!(i16::from_le_bytes([pkt[32], pkt[33]]), 0);
        assert_eq!(pkt[34], 2);
        assert_eq!(pkt[36], 7);
    }

    #[test]
    fn classifies_chu2_and_freedsp() {
        let c = classify_hid_device("/dev/hidraw4", 0x31B2, 0x0113, "MOONDROP CHU2 DSP").unwrap();
        assert_eq!(c.protocol, HidProtocol::KtMicro);
        assert_eq!(c.max_bands, 5);

        let f = classify_hid_device("/dev/hidraw5", 0x2FC6, 0xF001, "MOONDROP FreeDSP Pro").unwrap();
        assert_eq!(f.protocol, HidProtocol::Moondrop8);
        assert_eq!(f.max_bands, 8);
    }
}
