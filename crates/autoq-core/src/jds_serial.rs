//! USB-CDC Serial JSON protocol handler for JDS Labs Element IV (VID 0x152A, PID 0x88FA).
//!
//! Protocol format (null-terminated JSON over USB CDC serial at 115200 8N1):
//!   TX: `{"Product":"JDS Labs Element IV","Action":"Describe"}\0`
//!   TX: `{"Product":"JDS Labs Element IV","FormatOutput":true,"Action":"Update","Configuration":{"DSP":{"Headphone":{...}}}}\0`
//!   RX: null-terminated JSON response containing `Configuration.DSP.Headphone` or `"Status": true`.

use crate::biquad::{compute_safe_preamp_db, FilterBand, FilterType};
use crate::hid::{HidProtocol, MoondropDeviceInfo};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub const JDS_VID: u16 = 0x152A;
pub const JDS_ELEMENT_IV_PID: u16 = 0x88FA; // 35066 decimal

pub const FILTER_12_BAND_ORDER: [&str; 12] = [
    "Lowshelf 1",
    "Lowshelf 2",
    "Peaking 1",
    "Peaking 2",
    "Peaking 3",
    "Peaking 4",
    "Peaking 5",
    "Peaking 6",
    "Peaking 7",
    "Peaking 8",
    "Highshelf 1",
    "Highshelf 2",
];

pub fn jds_build_describe_payload() -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "Product": "JDS Labs Element IV",
        "Action": "Describe"
    }))
    .unwrap_or_default();
    bytes.push(0);
    bytes
}

/// Aligns arbitrary EQ bands into the fixed 12-band Element IV slot layout:
/// indices 0..=1 LowShelf, 2..=9 Peaking, 10..=11 HighShelf.
pub fn jds_align_12_bands(bands: &[FilterBand]) -> Vec<FilterBand> {
    let mut low_shelves = Vec::new();
    let mut peakings = Vec::new();
    let mut high_shelves = Vec::new();

    for b in bands {
        let clamped = b.clamped_with_max_gain(12.0);
        match clamped.filter_type {
            FilterType::LowShelf => {
                if low_shelves.len() < 2 {
                    low_shelves.push(clamped);
                }
            }
            FilterType::HighShelf => {
                if high_shelves.len() < 2 {
                    high_shelves.push(clamped);
                }
            }
            FilterType::Peaking => {
                if peakings.len() < 8 {
                    peakings.push(clamped);
                }
            }
        }
    }

    let mut aligned = Vec::with_capacity(12);
    for i in 0..2 {
        if let Some(b) = low_shelves.get(i) {
            aligned.push(b.clone());
        } else {
            aligned.push(FilterBand::new(FilterType::LowShelf, 80.0, 0.0, 0.707));
        }
    }
    for i in 0..8 {
        if let Some(b) = peakings.get(i) {
            aligned.push(b.clone());
        } else {
            aligned.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 0.707));
        }
    }
    for i in 0..2 {
        if let Some(b) = high_shelves.get(i) {
            aligned.push(b.clone());
        } else {
            aligned.push(FilterBand::new(FilterType::HighShelf, 10000.0, 0.0, 0.707));
        }
    }
    aligned
}

fn jds_type_str(ft: FilterType) -> &'static str {
    match ft {
        FilterType::LowShelf => "LOWSHELF",
        FilterType::Peaking => "PEAKING",
        FilterType::HighShelf => "HIGHSHELF",
    }
}

fn jds_parse_type(s: &str) -> FilterType {
    match s.trim().to_ascii_uppercase().as_str() {
        "LOWSHELF" | "LSQ" => FilterType::LowShelf,
        "HIGHSHELF" | "HSQ" => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

pub fn jds_build_update_payload(
    bands: &[FilterBand],
    preamp_db: f64,
    enable_eq: bool,
) -> Result<Vec<u8>, String> {
    let aligned = jds_align_12_bands(bands);
    let effective_preamp = if enable_eq {
        (preamp_db * 10.0).round() / 10.0
    } else {
        0.0
    };

    let mut headphone = serde_json::Map::new();
    headphone.insert(
        "Preamp".to_string(),
        json!({
            "Gain": effective_preamp,
            "Mode": "AUTO"
        }),
    );

    for (name, band) in FILTER_12_BAND_ORDER.iter().zip(aligned.iter()) {
        let g = if enable_eq { band.gain } else { 0.0 };
        headphone.insert(
            (*name).to_string(),
            json!({
                "Gain": (g * 10.0).round() / 10.0,
                "Frequency": band.frequency.round(),
                "Q": (band.q * 1000.0).round() / 1000.0,
                "Type": jds_type_str(band.filter_type),
            }),
        );
    }

    let payload = json!({
        "Product": "JDS Labs Element IV",
        "FormatOutput": true,
        "Action": "Update",
        "Configuration": {
            "DSP": {
                "Headphone": Value::Object(headphone)
            }
        }
    });

    let mut out = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    out.push(0);
    Ok(out)
}

pub fn jds_parse_describe_response(json_str: &str) -> Result<(f64, Vec<FilterBand>), String> {
    let trimmed = json_str.trim_matches('\0').trim();
    let v: Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("Respuesta JSON inválida de JDS Labs Element IV: {e}"))?;
    let hp = v
        .pointer("/Configuration/DSP/Headphone")
        .and_then(|x| x.as_object())
        .ok_or_else(|| "Respuesta Describe sin Configuration.DSP.Headphone".to_string())?;

    let preamp = hp
        .get("Preamp")
        .and_then(|p| p.pointer("/Gain/Current"))
        .and_then(|g| g.as_f64())
        .unwrap_or(0.0);

    let mut bands = Vec::with_capacity(12);
    for name in FILTER_12_BAND_ORDER {
        if let Some(fobj) = hp.get(name).and_then(|x| x.as_object()) {
            let ftype_str = fobj
                .get("Type")
                .and_then(|t| t.get("Current"))
                .and_then(|t| t.as_str())
                .or_else(|| fobj.get("Type").and_then(|t| t.as_str()))
                .unwrap_or_else(|| {
                    if name.starts_with("Lowshelf") {
                        "LOWSHELF"
                    } else if name.starts_with("Highshelf") {
                        "HIGHSHELF"
                    } else {
                        "PEAKING"
                    }
                });
            let freq = fobj
                .get("Frequency")
                .and_then(|x| x.get("Current"))
                .and_then(|x| x.as_f64())
                .or_else(|| fobj.get("Frequency").and_then(|x| x.as_f64()))
                .unwrap_or(1000.0);
            let gain = fobj
                .get("Gain")
                .and_then(|x| x.get("Current"))
                .and_then(|x| x.as_f64())
                .or_else(|| fobj.get("Gain").and_then(|x| x.as_f64()))
                .unwrap_or(0.0);
            let q = fobj
                .get("Q")
                .and_then(|x| x.get("Current"))
                .and_then(|x| x.as_f64())
                .or_else(|| fobj.get("Q").and_then(|x| x.as_f64()))
                .unwrap_or(0.707);
            bands.push(FilterBand::new(jds_parse_type(ftype_str), freq, gain, q).clamped());
        } else if name.starts_with("Lowshelf") {
            bands.push(FilterBand::new(FilterType::LowShelf, 80.0, 0.0, 0.707));
        } else if name.starts_with("Highshelf") {
            bands.push(FilterBand::new(FilterType::HighShelf, 10000.0, 0.0, 0.707));
        } else {
            bands.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 0.707));
        }
    }

    Ok((preamp, bands))
}

#[cfg(target_os = "linux")]
mod serial_imp {
    use super::*;
    use crate::posix_fd::PosixFd;
    use std::fs;
    use std::path::Path;

    fn read_hex_u16(path: &Path) -> Option<u16> {
        let s = fs::read_to_string(path).ok()?;
        u16::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
    }

    pub fn scan_jds_serial_devices() -> Vec<MoondropDeviceInfo> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir("/sys/class/tty") else {
            return out;
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with("ttyACM"))
            .collect();
        names.sort();

        for name in names {
            let tty_dev = Path::new("/sys/class/tty").join(&name).join("device");
            let Ok(canonical) = fs::canonicalize(&tty_dev) else {
                continue;
            };
            // On Linux CDC-ACM, `ttyACM*/device` points to the USB interface (e.g. `1-2:1.0`);
            // its parent directory contains `idVendor`, `idProduct`, and `product`.
            let Some(usb_dev_dir) = canonical.parent() else {
                continue;
            };
            let vid = read_hex_u16(&usb_dev_dir.join("idVendor")).unwrap_or(0);
            let pid = read_hex_u16(&usb_dev_dir.join("idProduct")).unwrap_or(0);
            let product = fs::read_to_string(usb_dev_dir.join("product"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let low = product.to_ascii_lowercase();

            if (vid == JDS_VID && pid == JDS_ELEMENT_IV_PID)
                || (vid == JDS_VID && low.contains("element iv"))
            {
                let dev_path = format!("/dev/{name}");
                if !Path::new(&dev_path).exists() {
                    continue;
                }
                let disp = if product.is_empty() {
                    "JDS Labs Element IV".to_string()
                } else {
                    product.clone()
                };
                out.push(MoondropDeviceInfo {
                    hidraw_path: dev_path,
                    vid,
                    pid,
                    hid_name: product,
                    display_name: disp,
                    protocol: HidProtocol::JdsElement,
                    max_bands: 12,
                    max_gain_db: 12.0,
                    supports_shelves: true,
                    disconnects_on_commit: false,
                    autoeq_hint: String::new(),
                    report_id: 0,
                    user_slot: 0,
                    bypass_slot: 0,
                    fiio_save_v2: false,
                });
            }
        }
        out
    }

    pub fn exchange_json(port_path: &str, tx_payload: &[u8]) -> Result<String, String> {
        let fd = PosixFd::open(port_path, libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK).map_err(
            |err| {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    format!(
                        "Sin permisos para abrir {port_path} (JDS Labs Element IV). Instala las reglas udev desde AVANZADO."
                    )
                } else {
                    format!("Error abriendo puerto serial {port_path}: {err}")
                }
            },
        )?;

        fd.configure_raw_serial_115200();

        // Write full payload
        let mut written = 0;
        while written < tx_payload.len() {
            match fd.write(&tx_payload[written..]) {
                Ok(n) => written += n,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(err) => return Err(format!("Error escribiendo serial JDS Labs: {err}")),
            }
        }

        // Read until `\0` terminator or 2000 ms timeout
        let deadline = Instant::now() + Duration::from_millis(2000);
        let mut rx = Vec::with_capacity(4096);
        let mut chunk = [0u8; 512];
        while Instant::now() < deadline {
            let rem = deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .clamp(1, 200) as i32;
            if !fd.wait_readable(rem) {
                continue;
            }
            if let Ok(n) = fd.read(&mut chunk) {
                if n > 0 {
                    let slice = &chunk[..n];
                    if let Some(pos) = slice.iter().position(|&b| b == 0) {
                        rx.extend_from_slice(&slice[..pos]);
                        return String::from_utf8(rx)
                            .map_err(|e| format!("UTF-8 inválido desde JDS Labs: {e}"));
                    }
                    rx.extend_from_slice(slice);
                }
            }
        }
        if !rx.is_empty() {
            return String::from_utf8(rx)
                .map_err(|e| format!("UTF-8 inválido desde JDS Labs: {e}"));
        }
        Err("Timeout esperando respuesta JSON de JDS Labs Element IV.".to_string())
    }
}

pub fn make_jds_device_info(port_path: String, hid_name: String) -> MoondropDeviceInfo {
    MoondropDeviceInfo {
        hidraw_path: port_path,
        vid: JDS_VID,
        pid: JDS_ELEMENT_IV_PID,
        hid_name,
        display_name: "JDS Labs Element IV".to_string(),
        protocol: HidProtocol::JdsElement,
        max_bands: 12,
        max_gain_db: 12.0,
        supports_shelves: true,
        disconnects_on_commit: false,
        autoeq_hint: String::new(),
        report_id: 0,
        user_slot: 0,
        bypass_slot: 0,
        fiio_save_v2: false,
    }
}

#[cfg(target_os = "windows")]
mod serial_imp {
    use super::*;
    use std::collections::HashSet;
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    pub fn scan_jds_serial_devices() -> Vec<MoondropDeviceInfo> {
        let mut out = Vec::new();
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Collect currently active COM port names from HKLM\HARDWARE\DEVICEMAP\SERIALCOMM
        let mut active_ports = HashSet::new();
        if let Ok(serialcomm) = hklm.open_subkey(r"HARDWARE\DEVICEMAP\SERIALCOMM") {
            for item in serialcomm.enum_values().flatten() {
                if let Ok(com) = serialcomm.get_value::<String, _>(&item.0) {
                    let norm = com.trim().to_ascii_uppercase();
                    if !norm.is_empty() {
                        active_ports.insert(norm);
                    }
                }
            }
        }
        if active_ports.is_empty() {
            return out;
        }

        // Match only currently active USB-CDC instances for VID_152A&PID_88FA (no subprocesses!)
        let Ok(usb_key) = hklm.open_subkey(r"SYSTEM\CurrentControlSet\Enum\USB\VID_152A&PID_88FA")
        else {
            return out;
        };
        let mut seen_ports = HashSet::new();
        for inst_name in usb_key.enum_keys().flatten() {
            let Ok(inst_key) = usb_key.open_subkey(&inst_name) else {
                continue;
            };
            let Ok(dev_params) = inst_key.open_subkey("Device Parameters") else {
                continue;
            };
            let Ok(port_name) = dev_params.get_value::<String, _>("PortName") else {
                continue;
            };
            let port_trim = port_name.trim().to_string();
            let port_upper = port_trim.to_ascii_uppercase();
            if !port_upper.starts_with("COM")
                || !active_ports.contains(&port_upper)
                || !seen_ports.insert(port_upper)
            {
                continue;
            }
            let friendly = inst_key
                .get_value::<String, _>("FriendlyName")
                .unwrap_or_else(|_| format!("JDS Labs Element IV ({port_trim})"));
            out.push(make_jds_device_info(
                format!(r"\\.\{port_trim}"),
                friendly.trim().to_string(),
            ));
        }
        out
    }

    pub fn exchange_json(port_path: &str, tx_payload: &[u8]) -> Result<String, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(port_path)
            .map_err(|e| format!("Error abriendo puerto serial {port_path}: {e}"))?;
        file.write_all(tx_payload)
            .map_err(|e| format!("Error escribiendo serial JDS Labs: {e}"))?;
        file.flush().ok();

        let mut rx = Vec::with_capacity(4096);
        let mut byte = [0u8; 1];
        let deadline = Instant::now() + Duration::from_millis(2000);
        while Instant::now() < deadline {
            match file.read(&mut byte) {
                Ok(0) => std::thread::sleep(Duration::from_millis(5)),
                Ok(_) => {
                    if byte[0] == 0 {
                        return String::from_utf8(rx)
                            .map_err(|e| format!("UTF-8 inválido desde JDS Labs: {e}"));
                    }
                    rx.push(byte[0]);
                }
                Err(e) => return Err(format!("Error leyendo serial JDS Labs: {e}")),
            }
        }
        if !rx.is_empty() {
            return String::from_utf8(rx)
                .map_err(|e| format!("UTF-8 inválido desde JDS Labs: {e}"));
        }
        Err("Timeout esperando respuesta JSON de JDS Labs Element IV.".to_string())
    }
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
mod serial_imp {
    use super::*;

    pub fn scan_jds_serial_devices() -> Vec<MoondropDeviceInfo> {
        Vec::new()
    }

    pub fn exchange_json(_port_path: &str, _tx_payload: &[u8]) -> Result<String, String> {
        Err("USB-CDC serial JDS Labs no soportado en esta plataforma.".to_string())
    }
}

pub use serial_imp::scan_jds_serial_devices;

pub fn read_jds_eq(info: &MoondropDeviceInfo) -> Result<(bool, Vec<FilterBand>), String> {
    let req = jds_build_describe_payload();
    let resp = serial_imp::exchange_json(&info.hidraw_path, &req)?;
    let (_preamp, bands) = jds_parse_describe_response(&resp)?;
    let enabled = bands.iter().any(|b| b.gain.abs() > 0.01);
    Ok((enabled, bands))
}

pub fn write_jds_eq(
    info: &MoondropDeviceInfo,
    bands: &[FilterBand],
    enable_eq: bool,
) -> Result<(), String> {
    let aligned = jds_align_12_bands(bands);
    let preamp = if enable_eq {
        compute_safe_preamp_db(&aligned)
    } else {
        0.0
    };
    let req = jds_build_update_payload(&aligned, preamp, enable_eq)?;
    let resp = serial_imp::exchange_json(&info.hidraw_path, &req)?;
    let v: Value = serde_json::from_str(resp.trim_matches('\0').trim())
        .map_err(|e| format!("Respuesta JSON inválida tras Update en JDS Labs: {e}"))?;
    if v.get("Status").and_then(|s| s.as_bool()) == Some(true) {
        Ok(())
    } else {
        Err(format!("JDS Labs Element IV rechazó la actualización EQ: {resp}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jds_aligns_bands_into_2lsq_8pk_2hsq() {
        let input = vec![
            FilterBand::new(FilterType::Peaking, 250.0, -2.0, 1.4),
            FilterBand::new(FilterType::LowShelf, 90.0, 4.0, 0.707),
            FilterBand::new(FilterType::HighShelf, 9000.0, 2.5, 0.707),
        ];
        let aligned = jds_align_12_bands(&input);
        assert_eq!(aligned.len(), 12);
        assert_eq!(aligned[0].filter_type, FilterType::LowShelf);
        assert!((aligned[0].gain - 4.0).abs() < 1e-6);
        assert_eq!(aligned[1].filter_type, FilterType::LowShelf);
        assert_eq!(aligned[1].gain, 0.0);
        assert_eq!(aligned[2].filter_type, FilterType::Peaking);
        assert!((aligned[2].frequency - 250.0).abs() < 1e-6);
        assert_eq!(aligned[10].filter_type, FilterType::HighShelf);
        assert!((aligned[10].gain - 2.5).abs() < 1e-6);
        assert_eq!(aligned[11].filter_type, FilterType::HighShelf);
    }

    #[test]
    fn jds_update_and_describe_json_roundtrip() {
        let bands = vec![
            FilterBand::new(FilterType::LowShelf, 105.0, 3.2, 0.707),
            FilterBand::new(FilterType::Peaking, 1000.0, -2.4, 1.5),
            FilterBand::new(FilterType::HighShelf, 10000.0, 1.6, 0.707),
        ];
        let describe = jds_build_describe_payload();
        assert_eq!(describe.last().copied(), Some(0));

        let payload = jds_build_update_payload(&bands, -3.2, true).unwrap();
        assert_eq!(payload.last().copied(), Some(0));
        let json_body = std::str::from_utf8(&payload[..payload.len() - 1]).unwrap();
        assert!(json_body.contains("\"Action\":\"Update\"") || json_body.contains("\"Action\": \"Update\""));

        let simulated_describe = json!({
            "Product": "JDS Labs Element IV",
            "Configuration": {
                "DSP": {
                    "Headphone": {
                        "Preamp": { "Gain": { "Current": -3.2 } },
                        "Lowshelf 1": { "Type": { "Current": "LOWSHELF" }, "Frequency": { "Current": 105 }, "Gain": { "Current": 3.2 }, "Q": { "Current": 0.707 } },
                        "Peaking 1": { "Type": { "Current": "PEAKING" }, "Frequency": { "Current": 1000 }, "Gain": { "Current": -2.4 }, "Q": { "Current": 1.5 } },
                        "Highshelf 1": { "Type": { "Current": "HIGHSHELF" }, "Frequency": { "Current": 10000 }, "Gain": { "Current": 1.6 }, "Q": { "Current": 0.707 } }
                    }
                }
            }
        });
        let (preamp, parsed) = jds_parse_describe_response(&simulated_describe.to_string()).unwrap();
        assert!((preamp - (-3.2)).abs() < 1e-6);
        assert_eq!(parsed.len(), 12);
        assert_eq!(parsed[0].filter_type, FilterType::LowShelf);
        assert!((parsed[0].gain - 3.2).abs() < 1e-6);
        assert_eq!(parsed[2].filter_type, FilterType::Peaking);
        assert!((parsed[2].gain - (-2.4)).abs() < 1e-6);
        assert_eq!(parsed[10].filter_type, FilterType::HighShelf);
        assert!((parsed[10].gain - 1.6).abs() < 1e-6);
    }

    #[test]
    fn jds_rejects_malformed_or_incomplete_describe_json() {
        assert!(jds_parse_describe_response("").is_err());
        assert!(jds_parse_describe_response("{not json").is_err());
        assert!(jds_parse_describe_response(r#"{"Product":"JDS"}"#).is_err());
    }
}
