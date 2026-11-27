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
fn ktmicro_golden_wire_frames() {
    // Enable slot ON (`0x03`) / bypass OFF (`0x02`) + Flash commit (`0x53`)
    let slot_on = kt_packet(KT_REG_SLOT, KT_WRITE, [KT_SLOT_ON, 0, 0, 0]);
    assert_eq!(
        slot_on,
        [0x4B, 0x24, 0x00, 0x00, 0x00, 0x57, 0x00, 0x03, 0x00, 0x00, 0x00]
    );
    let slot_off = kt_packet(KT_REG_SLOT, KT_WRITE, [KT_SLOT_OFF, 0, 0, 0]);
    assert_eq!(
        slot_off,
        [0x4B, 0x24, 0x00, 0x00, 0x00, 0x57, 0x00, 0x02, 0x00, 0x00, 0x00]
    );
    let commit = kt_packet(0x00, KT_COMMIT, [0, 0, 0, 0]);
    assert_eq!(commit[0], 0x4B);
    assert_eq!(commit[5], 0x53);

    // Read request for Band 2 (`reg = 0x26 + 2*2 = 0x2A`)
    let read_req = kt_packet(KT_REG_BAND0 + 4, KT_READ, [0, 0, 0, 0]);
    assert_eq!(
        read_req,
        [0x4B, 0x2A, 0x00, 0x00, 0x00, 0x52, 0x00, 0x00, 0x00, 0x00, 0x00]
    );

    // Peaking 1000 Hz, -3.5 dB (`-35` = `0xFFDD`), Q=1.410 (`1410` = `0x0582`)
    let band = FilterBand::new(FilterType::Peaking, 1000.0, -3.5, 1.41);
    let (gf, qt) = kt_encode_band(&band);
    let tx_gf = kt_packet(KT_REG_BAND0 + 2, KT_WRITE, gf);
    let tx_qt = kt_packet(KT_REG_BAND0 + 3, KT_WRITE, qt);
    assert_eq!(
        tx_gf,
        [0x4B, 0x28, 0x00, 0x00, 0x00, 0x57, 0x00, 0xDD, 0xFF, 0xE8, 0x03]
    );
    assert_eq!(
        tx_qt,
        [0x4B, 0x29, 0x00, 0x00, 0x00, 0x57, 0x00, 0x82, 0x05, 0x00, 0x00]
    );

    let decoded = kt_decode_band(gf, qt);
    assert_eq!(decoded.filter_type, FilterType::Peaking);
    assert_eq!(decoded.frequency, 1000.0);
    assert!((decoded.gain - (-3.5)).abs() < 1e-6);
    assert!((decoded.q - 1.41).abs() < 1e-6);
}

#[test]
fn moondrop8_and_walkplay_golden_frames() {
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

    // WalkPlay custom user slot 101 with LowShelf 120 Hz, +3.5 dB, Q 0.707
    let wp_band = FilterBand::new(FilterType::LowShelf, 120.0, 3.5, 0.707);
    let wp_pkt = md8_build_write_packet_with_slot(3, &wp_band, 101);
    assert_eq!(&wp_pkt[0..6], &[0x4B, 0x01, 0x09, 0x18, 0x00, 0x03]);
    assert_eq!(u16::from_le_bytes([wp_pkt[28], wp_pkt[29]]), 120);
    assert_eq!(u16::from_le_bytes([wp_pkt[30], wp_pkt[31]]), 181); // (0.707 * 256).round()
    assert_eq!(i16::from_le_bytes([wp_pkt[32], wp_pkt[33]]), 896); // (3.5 * 256).round()
    assert_eq!(wp_pkt[34], 1); // LowShelf
    assert_eq!(wp_pkt[36], 101); // WalkPlay user_slot
    assert_eq!(md8_code_to_type(wp_pkt[34]), FilterType::LowShelf);
}

#[test]
fn fiio_filter_packet_big_endian_roundtrip() {
    let band = FilterBand::new(FilterType::HighShelf, 8500.0, -4.5, 1.41);
    let pkt = fiio_build_set_filter_packet(7, 3, &band);
    assert_eq!(
        pkt,
        [
            7, 0xAA, 0x0A, 0x00, 0x00, 0x15, 0x08, 3, 0xFF, 0xD3, // -45 i16 BE
            0x21, 0x34, // 8500 u16 BE
            0x00, 0x8D, // 141 u16 BE
            0x02, // HighShelf
            0x00, 0xEE
        ]
    );

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

    let q5k = classify_hid_device("/dev/hidraw10", 0x0A12, 0x4125, "Qudelix-5K USB DAC 96KHz").unwrap();
    assert_eq!(q5k.protocol, HidProtocol::Qudelix);
    assert_eq!(q5k.max_bands, 10);

    let top = classify_hid_device("/dev/hidraw11", 0x152A, 0x8750, "TOPPING DX1 II").unwrap();
    assert_eq!(top.protocol, HidProtocol::Topping);
    assert_eq!(top.max_bands, 11);

    let fosi = classify_hid_device("/dev/hidraw12", 0x152A, 0x8700, "Fosi Audio DS3").unwrap();
    assert_eq!(fosi.protocol, HidProtocol::Fosi);
    assert_eq!(fosi.max_bands, 8);

    let freedsp_cx = classify_hid_device("/dev/hidraw14", 0x35D8, 0x1496, "Moondrop FreeDSP").unwrap();
    assert_eq!(freedsp_cx.protocol, HidProtocol::Conexant);
    assert_eq!(freedsp_cx.max_bands, 9);

    let echob = classify_hid_device("/dev/hidraw15", 0x35D8, 0x149B, "Moondrop ECHO-B").unwrap();
    assert_eq!(echob.protocol, HidProtocol::Conexant);
    assert_eq!(echob.max_bands, 9);

    // FreeDSP Pro on WalkPlay VID must remain Moondrop8, not Conexant
    let fpro = classify_hid_device("/dev/hidraw16", 0x35D8, 0xF001, "Moondrop FreeDSP Pro").unwrap();
    assert_eq!(fpro.protocol, HidProtocol::Moondrop8);

    // Generic 0x152A without Topping/Fosi string must not false-positive
    assert!(classify_hid_device("/dev/hidraw13", 0x152A, 0x1234, "Generic XMOS Audio").is_none());
}

#[test]
fn conexant_packet_layout_and_usage_summary() {
    let band = FilterBand::new(FilterType::LowShelf, 100.0, 3.5, 0.9);
    let pkts = conexant_build_band_packets(1, &band);
    assert_eq!(pkts.len(), 5);
    assert_eq!(pkts[0].len(), 62);
    assert_eq!(pkts[0][0], CONEXANT_REPORT_ID);
    assert_eq!(&pkts[0][1..4], &[1, 1, 0]);
    assert_eq!(
        u32::from_le_bytes(pkts[0][8..12].try_into().unwrap()),
        CONEXANT_MAGIC
    );
    // Flat filter @ gain 0 => b0 = 2^30, b1 = -a1_neg, b2 = -a2_neg
    let flat = FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0);
    let bq = conexant_compute_biquad(&flat, 48_000.0);
    assert_eq!(bq[0], 1_073_741_824);
    assert_eq!(bq[1], -bq[3]);
    assert_eq!(bq[2], -bq[4]);

    let (up, u) = parse_hid_usage_summary(&[0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01]);
    assert_eq!(up, Some(0xFF00));
    assert_eq!(u, Some(0x01));
}

#[test]
fn fosi_ds3_packet_float32_roundtrip() {
    let band = FilterBand::new(FilterType::LowShelf, 105.0, -3.5, 0.707);
    let pkt = fosi_encode_band_packet(1, 7, 2, &band);
    assert_eq!(&pkt[0..6], &[0x01, 0x77, 0x8D, 7, 2, 9]);
    // Verify exact IEEE-754 float32 LE payload bytes: freq=105.0, q=0.707, bw=0.0, gain=-3.5
    assert_eq!(&pkt[6..10], &105.0_f32.to_le_bytes());
    assert_eq!(&pkt[10..14], &0.707_f32.to_le_bytes());
    assert_eq!(&pkt[14..18], &0.0_f32.to_le_bytes());
    assert_eq!(&pkt[18..22], &(-3.5_f32).to_le_bytes());

    let (idx, dec, disabled) = fosi_decode_band_packet(&pkt).unwrap();
    assert!(!disabled);
    assert_eq!(idx, 2);
    assert_eq!(dec.filter_type, FilterType::LowShelf);
    assert!((dec.frequency - 105.0).abs() < 0.01);
    assert!((dec.gain - (-3.5)).abs() < 0.01);
    assert!((dec.q - 0.707).abs() < 0.01);
}

#[test]
fn topping_frame_crc_and_packed_band_roundtrip() {
    let pkt = topping_build_frame(TOPPING_PTYPE_WRITE_NACK, 0x9E01, 0);
    assert_eq!(pkt[0], 0x00);
    assert_eq!(pkt[1], 0x22);
    assert_eq!(pkt[2], 0x33);
    assert_eq!(pkt[14], 0x66);
    assert_eq!(pkt[15], 0x77);
    assert_eq!(pkt[16], 0x00);
    let parsed = topping_parse_frame(&pkt).unwrap();
    assert_eq!(parsed.cmd, 0x9E01);
    assert_eq!(parsed.protocol_type, 32);

    let crc_good = topping_crc16_modbus(&pkt[1..17]);
    assert_ne!(crc_good, 0);
    let mut bad_pkt = pkt;
    bad_pkt[5] ^= 0x01;
    assert_ne!(topping_crc16_modbus(&bad_pkt[1..17]), crc_good);

    let gain_i8 = topping_encode_gain_byte(-4.2) as i8 as u8;
    let packed = (1u32) | (2u32 << 8) | ((gain_i8 as u32) << 16);
    let (en, dec) = topping_decode_packed_band(packed, 105, 7070);
    assert!(en);
    assert_eq!(dec.filter_type, FilterType::LowShelf);
    assert_eq!(dec.frequency, 105.0);
    assert!((dec.gain - (-4.2)).abs() < 1e-6);
    assert!((dec.q - 0.707).abs() < 1e-4);
}

#[test]
fn qudelix_packet_and_v3_bitstream_parser() {
    let band = FilterBand::new(FilterType::HighShelf, 8000.0, 2.5, 0.707);
    let pkt = qudelix_build_band_packet(8, 3, &band);
    assert_eq!(&pkt[0..9], &[8, 13, 0x80, 0x07, 0x0F, 0x00, 0x03, 3, 4]);

    // Build an 88-byte V3 preset bitstream with Band 0 = Peak 1000 Hz, +3.0 dB, Q=1.0
    // and Band 1 = LowShelf 120 Hz, -2.5 dB, Q=0.707
    let mut buf = [0u8; 88];
    let write_bits = |b: &mut [u8], bit_off: usize, n: usize, val: u32| {
        for i in 0..n {
            if ((val >> i) & 1) != 0 {
                let pos = bit_off + i;
                b[pos >> 3] |= 1 << (pos & 7);
            }
        }
    };
    write_bits(&mut buf, 32, 16, (-30_i16 as u16) as u32);
    write_bits(&mut buf, 64, 16, 1000);
    write_bits(&mut buf, 64 + 16, 16, 120);
    // Band 0 params at bit 384
    write_bits(&mut buf, 384, 4, 5);
    write_bits(&mut buf, 384 + 4, 10, 30);
    write_bits(&mut buf, 384 + 14, 14, 1024);
    // Band 1 params at bit 384 + 32 = 416: type=3 (LowShelf), gain=-25 (10-bit signed), q=724 (0.707*1024)
    write_bits(&mut buf, 416, 4, 3);
    write_bits(&mut buf, 416 + 4, 10, ((-25_i32) & 0x3FF) as u32);
    write_bits(&mut buf, 416 + 14, 14, 724);

    let (preamp, bands) = qudelix_parse_user_eq_preset(&buf).unwrap();
    assert!((preamp - (-3.0)).abs() < 1e-6);
    assert_eq!(bands.len(), 10);
    assert_eq!(bands[0].filter_type, FilterType::Peaking);
    assert_eq!(bands[0].frequency, 1000.0);
    assert!((bands[0].gain - 3.0).abs() < 1e-6);
    assert!((bands[0].q - 1.0).abs() < 0.01);
    assert_eq!(bands[1].filter_type, FilterType::LowShelf);
    assert_eq!(bands[1].frequency, 120.0);
    assert!((bands[1].gain - (-2.5)).abs() < 1e-6);
    assert!((bands[1].q - 0.707).abs() < 0.01);
}

#[test]
fn rejects_truncated_and_malformed_hid_packets() {
    assert!(fiio_decode_filter_payload(&[]).is_none());
    assert!(fiio_decode_filter_payload(&[0xBB, 0x0B, 0x00, 0x00, 0x15]).is_none());
    assert!(fiio_decode_filter_payload(&[0xAA; 16]).is_none());

    assert!(fosi_decode_band_packet(&[]).is_none());
    assert!(fosi_decode_band_packet(&[0x77, 0x8D, 0x00, 0x01]).is_none());
    let mut nan_fosi = [0u8; 24];
    nan_fosi[0] = 0x77;
    nan_fosi[4] = 2;
    nan_fosi[5..9].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(fosi_decode_band_packet(&nan_fosi).is_none());

    assert!(topping_parse_frame(&[]).is_none());
    assert!(topping_parse_frame(&[0x22, 0x33, 0x20, 0x01, 0x01]).is_none());
    let mut bad_end = [0u8; 16];
    bad_end[0] = 0x22;
    bad_end[1] = 0x33;
    bad_end[13] = 0x00;
    assert!(topping_parse_frame(&bad_end).is_none());

    assert!(qudelix_parse_user_eq_preset(&[0u8; 40]).is_err());
}
