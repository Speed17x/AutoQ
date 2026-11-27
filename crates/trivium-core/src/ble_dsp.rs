//! Wireless BLE GATT DSP protocol codecs & device catalog for Trivium:
//! 1. **FiiO BLE GATT** (`FIIO EH11`, `FIIO EH13`): 10-band PEQ, ±20 dB gain,
//!    Service `00001100-04a5-1000-1000-40ed981a04a5`.
//! 2. **Airoha BLE GATT** (`Audeze Maxwell`): 10-band PEQ, 4 writable slots (0–3),
//!    Service `5052494d-2dab-0341-6972-6f6861424c45`.
//!
//! Protocol layouts verified against `jeromeof/devicePEQ` (`fiioBleHandler.js`,
//! `airohaBleHandler.js`, and `bluetoothBleDeviceConfig.js`).

use crate::biquad::{FilterBand, FilterType};
use serde::{Deserialize, Serialize};

pub const FIIO_BLE_SERVICE_UUID: &str = "00001100-04a5-1000-1000-40ed981a04a5";
pub const FIIO_BLE_TX_UUID: &str = "00001101-04a5-1000-1000-40ed981a04a5";
pub const FIIO_BLE_RX_UUID: &str = "00001102-04a5-1000-1000-40ed981a04a5";

pub const AIROHA_BLE_SERVICE_UUID: &str = "5052494d-2dab-0341-6972-6f6861424c45";
pub const AIROHA_BLE_TX_UUID: &str = "43484152-2dab-3241-6972-6f6861424c45";
pub const AIROHA_BLE_RX_UUID: &str = "43484152-2dab-3141-6972-6f6861424c45";

pub const AIROHA_READ_RESPONSE_LEN: usize = 193;
pub const BLE_NUM_BANDS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BleDspProtocol {
    FiioBle,
    AirohaBle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleGattProfileSpec {
    pub protocol: BleDspProtocol,
    pub display_name: String,
    pub name_prefixes: Vec<String>,
    pub service_uuid: String,
    pub tx_char_uuid: String,
    pub rx_char_uuid: String,
    pub max_bands: usize,
    pub max_gain_db: f64,
    pub slots: usize,
}

pub fn supported_ble_profiles() -> Vec<BleGattProfileSpec> {
    vec![
        BleGattProfileSpec {
            protocol: BleDspProtocol::FiioBle,
            display_name: "FiiO BLE GATT (EH11 / EH13)".to_string(),
            name_prefixes: vec!["FIIO EH11".to_string(), "FIIO EH13".to_string(), "FIIO".to_string()],
            service_uuid: FIIO_BLE_SERVICE_UUID.to_string(),
            tx_char_uuid: FIIO_BLE_TX_UUID.to_string(),
            rx_char_uuid: FIIO_BLE_RX_UUID.to_string(),
            max_bands: BLE_NUM_BANDS,
            max_gain_db: 20.0,
            slots: 1,
        },
        BleGattProfileSpec {
            protocol: BleDspProtocol::AirohaBle,
            display_name: "Airoha BLE GATT (Audeze Maxwell)".to_string(),
            name_prefixes: vec!["Audeze Maxwell".to_string()],
            service_uuid: AIROHA_BLE_SERVICE_UUID.to_string(),
            tx_char_uuid: AIROHA_BLE_TX_UUID.to_string(),
            rx_char_uuid: AIROHA_BLE_RX_UUID.to_string(),
            max_bands: BLE_NUM_BANDS,
            max_gain_db: 12.0,
            slots: 4,
        },
    ]
}

// ---------------------------------------------------------------------------
// 1. FiiO BLE GATT Codec (`F1 10 00 LL CC CC [payload] FF`)
// ---------------------------------------------------------------------------

pub fn fiio_ble_type_to_byte(ft: FilterType) -> u8 {
    match ft {
        FilterType::Peaking => 0,
        FilterType::LowShelf => 1,
        FilterType::HighShelf => 2,
    }
}

pub fn fiio_ble_byte_to_type(code: u8) -> FilterType {
    match code {
        1 => FilterType::LowShelf,
        2 => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

pub fn fiio_ble_build_packet(cmd1: u8, cmd2: u8, payload: &[u8]) -> Vec<u8> {
    let total = 2 + 2 + 2 + payload.len() + 1;
    let mut out = Vec::with_capacity(total);
    out.push(0xF1);
    out.push(0x10);
    out.push(((total >> 8) & 0xFF) as u8);
    out.push((total & 0xFF) as u8);
    out.push(cmd1);
    out.push(cmd2);
    out.extend_from_slice(payload);
    out.push(0xFF);
    out
}

pub fn fiio_ble_handshake_packet() -> Vec<u8> {
    fiio_ble_build_packet(0x00, 0x02, &[0x01])
}

pub fn fiio_ble_read_eq_packet() -> Vec<u8> {
    fiio_ble_build_packet(0x03, 0x0D, &[0x01, 0x00, 0x09])
}

pub fn fiio_ble_battery_packet() -> Vec<u8> {
    fiio_ble_build_packet(0x00, 0x03, &[0x01])
}

pub fn fiio_ble_set_enabled_packet(enabled: bool) -> Vec<u8> {
    fiio_ble_build_packet(0x13, 0x01, &[0x01, if enabled { 0x01 } else { 0x00 }])
}

pub fn fiio_ble_write_band_packet(band_idx: u8, band: &FilterBand) -> Vec<u8> {
    let b = band.clamped_with_max_gain(20.0);
    let gain_raw = (b.gain * 10.0).round().clamp(-200.0, 200.0) as i16;
    let freq_raw = b.frequency.round().clamp(20.0, 20000.0) as u16;
    let q_raw = (b.q * 100.0).round().clamp(10.0, 1000.0) as u16;
    let ftype = fiio_ble_type_to_byte(b.filter_type);
    let gb = gain_raw.to_be_bytes();
    let fb = freq_raw.to_be_bytes();
    let qb = q_raw.to_be_bytes();
    // F1 10 00 11  13 0D  01 [idx][idx] [g_hi g_lo f_hi f_lo q_hi q_lo type]  FF
    fiio_ble_build_packet(
        0x13,
        0x0D,
        &[
            0x01, band_idx, band_idx, gb[0], gb[1], fb[0], fb[1], qb[0], qb[1], ftype,
        ],
    )
}

pub fn fiio_ble_parse_eq_response(data: &[u8]) -> Option<Vec<FilterBand>> {
    if data.len() < 80 || data[0] != 0xF1 || data[1] != 0x10 {
        return None;
    }
    if data[4] != 0x03 || data[5] != 0x0D {
        return None;
    }
    let mut bands = Vec::with_capacity(BLE_NUM_BANDS);
    let base = 9;
    for i in 0..BLE_NUM_BANDS {
        let o = base + i * 7;
        if o + 7 > data.len() - 1 {
            break;
        }
        let gain_raw = i16::from_be_bytes([data[o], data[o + 1]]);
        let freq_raw = u16::from_be_bytes([data[o + 2], data[o + 3]]);
        let q_raw = u16::from_be_bytes([data[o + 4], data[o + 5]]);
        let ftype = fiio_ble_byte_to_type(data[o + 6]);
        let band = FilterBand::new(
            ftype,
            (freq_raw as f64).clamp(20.0, 20000.0),
            (gain_raw as f64) / 10.0,
            ((q_raw as f64) / 100.0).max(0.1),
        )
        .clamped_with_max_gain(20.0);
        bands.push(band);
    }
    if bands.len() == BLE_NUM_BANDS {
        Some(bands)
    } else {
        None
    }
}

pub fn fiio_ble_parse_battery(data: &[u8]) -> Option<u8> {
    if data.len() < 9 || data[0] != 0xF1 || data[1] != 0x10 || data[4] != 0x00 || data[5] != 0x03
    {
        return None;
    }
    Some(data[7].min(100))
}

// ---------------------------------------------------------------------------
// 2. Airoha BLE GATT Codec (`05 5A ...` / `05 5B BD ...`, 10 bands, 193B)
// ---------------------------------------------------------------------------

pub fn airoha_ble_read_preset_packet(preset: u8) -> [u8; 10] {
    [0x05, 0x5A, 0x06, 0x00, 0x00, 0x0A, preset & 0x03, 0xEF, 0xE8, 0x03]
}

pub fn airoha_ble_type_to_byte(ft: FilterType) -> u8 {
    match ft {
        FilterType::Peaking => 2,
        FilterType::LowShelf => 3,
        FilterType::HighShelf => 4,
    }
}

pub fn airoha_ble_byte_to_type(code: u8) -> FilterType {
    match code {
        3 => FilterType::LowShelf,
        4 => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

pub fn airoha_ble_write_peq_packet(preset_num: u8, bands: &[FilterBand]) -> Vec<u8> {
    let mut fitted: Vec<FilterBand> = bands.iter().take(BLE_NUM_BANDS).map(FilterBand::clamped).collect();
    while fitted.len() < BLE_NUM_BANDS {
        fitted.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
    }

    let _ = preset_num & 0x03;
    let mut cmd = Vec::with_capacity(AIROHA_READ_RESPONSE_LEN);
    // Header: 05 5A BD  00 01  0A 00 EF 01 00 00 00 00
    cmd.extend_from_slice(&[
        0x05, 0x5A, 0xBD, 0x00, 0x01, 0x0A, 0x00, 0xEF, 0x01, 0x00, 0x00, 0x00, 0x00,
    ]);

    for b in fitted {
        cmd.push(0x01);
        cmd.push(airoha_ble_type_to_byte(b.filter_type));
        let freq_val = (b.frequency * 100.0).round() as u32;
        let gain_val = (b.gain * 100.0).round() as i32;
        let bw_val = 0u32;
        let q_val = (b.q * 100.0).round() as u32;
        cmd.extend_from_slice(&freq_val.to_le_bytes());
        cmd.extend_from_slice(&gain_val.to_le_bytes());
        cmd.extend_from_slice(&bw_val.to_le_bytes());
        cmd.extend_from_slice(&q_val.to_le_bytes());
    }
    cmd
}

pub fn airoha_ble_parse_peq_response(data: &[u8]) -> Option<(bool, Vec<FilterBand>)> {
    if data.len() < AIROHA_READ_RESPONSE_LEN {
        return None;
    }
    if data[0] != 0x05 || data[1] != 0x5B || data[2] != 0xBD {
        return None;
    }
    let num_bands = (data[5] as usize).min(BLE_NUM_BANDS);
    if num_bands == 0 {
        return None;
    }
    let eq_enabled = data[8] == 1;
    let mut bands = Vec::with_capacity(BLE_NUM_BANDS);
    let filter_start = 13usize;
    for i in 0..num_bands {
        let o = filter_start + i * 18;
        if o + 18 > data.len() {
            break;
        }
        let ftype = airoha_ble_byte_to_type(data[o + 1]);
        let freq_raw = u32::from_le_bytes([data[o + 2], data[o + 3], data[o + 4], data[o + 5]]);
        let gain_raw = i32::from_le_bytes([data[o + 6], data[o + 7], data[o + 8], data[o + 9]]);
        let q_raw = u32::from_le_bytes([data[o + 14], data[o + 15], data[o + 16], data[o + 17]]);
        bands.push(
            FilterBand::new(
                ftype,
                (freq_raw as f64) / 100.0,
                (gain_raw as f64) / 100.0,
                ((q_raw as f64) / 100.0).max(0.1),
            )
            .clamped(),
        );
    }
    if bands.is_empty() {
        return None;
    }
    while bands.len() < BLE_NUM_BANDS {
        bands.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
    }
    Some((eq_enabled, bands))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fiio_ble_packet_and_response_roundtrip() {
        let b0 = FilterBand::new(FilterType::LowShelf, 105.0, 4.5, 0.71);
        let pkt = fiio_ble_write_band_packet(0, &b0);
        assert_eq!(pkt.len(), 17);
        assert_eq!(pkt[0], 0xF1);
        assert_eq!(pkt[1], 0x10);
        assert_eq!(pkt[4], 0x13);
        assert_eq!(pkt[5], 0x0D);
        assert_eq!(pkt[16], 0xFF);

        // Build an 80-byte response packet simulating CMD 03 0D
        let mut resp = vec![0u8; 80];
        resp[0] = 0xF1;
        resp[1] = 0x10;
        resp[2] = 0x00;
        resp[3] = 80;
        resp[4] = 0x03;
        resp[5] = 0x0D;
        resp[79] = 0xFF;
        // Band 0 at offset 9: take bytes 9..16 from write packet ([g_hi g_lo f_hi f_lo q_hi q_lo type])
        resp[9..16].copy_from_slice(&pkt[9..16]);
        for i in 1..10 {
            let o = 9 + i * 7;
            let f_be = 1000u16.to_be_bytes();
            let q_be = 100u16.to_be_bytes();
            resp[o + 2..o + 4].copy_from_slice(&f_be);
            resp[o + 4..o + 6].copy_from_slice(&q_be);
        }
        let parsed = fiio_ble_parse_eq_response(&resp).unwrap();
        assert_eq!(parsed.len(), 10);
        assert_eq!(parsed[0].filter_type, FilterType::LowShelf);
        assert_eq!(parsed[0].frequency, 105.0);
        assert!((parsed[0].gain - 4.5).abs() < 0.01);
        assert!((parsed[0].q - 0.71).abs() < 0.01);
    }

    #[test]
    fn airoha_ble_write_and_parse_roundtrip() {
        let mut bands = vec![FilterBand::new(FilterType::LowShelf, 95.0, -3.2, 0.75)];
        for _ in 1..10 {
            bands.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
        }
        let pkt = airoha_ble_write_peq_packet(0, &bands);
        assert_eq!(pkt.len(), AIROHA_READ_RESPONSE_LEN);
        assert_eq!(&pkt[0..3], &[0x05, 0x5A, 0xBD]);

        // Transform TX header to RX response header `05 5B BD`
        let mut rx = pkt.clone();
        rx[1] = 0x5B;
        let (enabled, decoded) = airoha_ble_parse_peq_response(&rx).unwrap();
        assert!(enabled);
        assert_eq!(decoded.len(), 10);
        assert_eq!(decoded[0].filter_type, FilterType::LowShelf);
        assert!((decoded[0].frequency - 95.0).abs() < 0.02);
        assert!((decoded[0].gain - (-3.2)).abs() < 0.02);
        assert!((decoded[0].q - 0.75).abs() < 0.02);
    }

    #[test]
    fn ble_parsers_reject_malformed_packets() {
        assert!(fiio_ble_parse_eq_response(&[]).is_none());
        assert!(fiio_ble_parse_eq_response(&[0xF1; 40]).is_none());
        assert!(fiio_ble_parse_battery(&[0xF1, 0x10]).is_none());
        assert!(airoha_ble_parse_peq_response(&[]).is_none());
        assert!(airoha_ble_parse_peq_response(&[0x05; 100]).is_none());
    }
}
