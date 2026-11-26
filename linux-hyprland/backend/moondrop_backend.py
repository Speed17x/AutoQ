#!/usr/bin/env python3
"""Universal AutoQ JSON CLI backend for KDE Plasma 6 and Hyprland Nothing OS widgets.

Full feature parity with AutoQ Android:
  - USB HID direct DSP (KTMicro 5-band & Moondrop 8-band)
  - 10-band PipeWire filter-chain PEQ for ANY Bluetooth, USB-C, or wired 3.5mm audio output
  - 8 open FR databases (AutoEq GitHub + 7 Squiglink repositories, 13,000+ curves)
  - Universal source & target headphone search + 8 curated target curves
  - EqualizerAPO ParametricEQ.txt import/export, universal presets, optional BT SPP
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))

import autoeq_db as db
from biquad import FilterBand
import chu2_hid
import moondrop_bt_pw as btpw

CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "chu2-dsp-widget"
PRESETS_FILE = CONFIG_DIR / "presets.json"
DEFAULT_VIRTUAL_BANDS = 10

HARMAN_WARM_BANDS = [
    FilterBand("low_shelf", 105.0, 3.2, 0.707),
    FilterBand("peaking", 240.0, -1.4, 1.100),
    FilterBand("peaking", 1000.0, 0.0, 1.000),
    FilterBand("peaking", 2800.0, 1.8, 1.400),
    FilterBand("peaking", 6200.0, -1.6, 3.200),
    FilterBand("high_shelf", 9500.0, 1.2, 0.707),
]

VOCAL_REF_BANDS = [
    FilterBand("low_shelf", 85.0, -1.5, 0.707),
    FilterBand("peaking", 320.0, -1.0, 1.200),
    FilterBand("peaking", 1600.0, 2.2, 1.300),
    FilterBand("peaking", 3600.0, 1.5, 1.800),
    FilterBand("high_shelf", 10000.0, 0.8, 0.707),
]

BASS_BOOST_BANDS = [
    FilterBand("low_shelf", 90.0, 4.5, 0.707),
    FilterBand("peaking", 220.0, -1.0, 1.200),
    FilterBand("peaking", 1000.0, 0.0, 1.000),
    FilterBand("peaking", 3200.0, 0.8, 1.400),
    FilterBand("high_shelf", 10000.0, 0.5, 0.707),
]


def _flat_bands(n: int = DEFAULT_VIRTUAL_BANDS) -> List[FilterBand]:
    freqs = (
        [60.0, 250.0, 1000.0, 3500.0, 8000.0]
        if n <= 5
        else [31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0]
    )
    return [FilterBand("peaking", freqs[i % len(freqs)], 0.0, 1.0) for i in range(max(1, n))]


def _fit_band_count(bands: Sequence[FilterBand], n: int) -> List[FilterBand]:
    out = [b.clamped() for b in list(bands)[:n]]
    pad = _flat_bands(n)
    while len(out) < n:
        out.append(pad[len(out)])
    return out


def _default_presets() -> Dict[str, List[dict]]:
    return {
        "Plano (0 dB)": [vars(b) for b in _flat_bands(DEFAULT_VIRTUAL_BANDS)],
        "Harman Warm": [vars(b) for b in HARMAN_WARM_BANDS],
        "Vocal / Ref": [vars(b) for b in VOCAL_REF_BANDS],
        "Bass Boost": [vars(b) for b in BASS_BOOST_BANDS],
    }


def _load_presets() -> Dict[str, List[dict]]:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    defaults = _default_presets()
    if not PRESETS_FILE.exists():
        PRESETS_FILE.write_text(json.dumps(defaults, indent=2, ensure_ascii=False), encoding="utf-8")
        return defaults
    try:
        data = json.loads(PRESETS_FILE.read_text(encoding="utf-8"))
        if isinstance(data, dict):
            # Migrate old Chu 2 factory built-in out cleanly, ensure universal defaults exist
            changed = False
            for old_key in ("Chu 2 DSP (Fábrica)", "Plano / Stock sin EQ (0 dB)", "Plano / 3.5mm Stock (0 dB)"):
                if old_key in data:
                    del data[old_key]
                    changed = True
            for k, v in defaults.items():
                if k not in data:
                    data[k] = v
                    changed = True
            if changed:
                _save_presets(data)
            return data
    except Exception:
        pass
    return defaults


def _save_presets(data: Dict[str, List[dict]]) -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    PRESETS_FILE.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")


def _discover_all_devices() -> List[Dict[str, Any]]:
    devices: List[Dict[str, Any]] = []
    usb_objs = chu2_hid.scan_moondrop_devices()
    hid_vid_pids = [d.vid_pid for d in usb_objs]

    for d in usb_objs:
        devices.append({
            "id": f"usb:{d.hidraw_path}",
            "kind": "usb_hid",
            "display_name": d.display_name,
            "transport_label": f"HID·{d.max_bands}B",
            "max_bands": d.max_bands,
            "max_gain_db": d.max_gain_db,
            "supports_shelves": d.supports_shelves,
            "supports_spp": False,
            "autoeq_hint": d.autoeq_hint,
            "hidraw_path": d.hidraw_path,
            "vid_pid": d.vid_pid,
            "_usb_obj": d,
        })

    for p in btpw.scan_pipewire_audio_sinks(hid_vid_pids):
        devices.append({
            "id": f"pw:{p.sink_name}",
            "kind": p.kind,
            "display_name": p.display_name,
            "transport_label": p.transport_badge,
            "max_bands": p.max_bands,
            "max_gain_db": p.max_gain_db,
            "supports_shelves": p.supports_shelves,
            "supports_spp": p.supports_spp,
            "autoeq_hint": p.autoeq_hint,
            "sink_name": p.sink_name,
            "mac": p.mac,
            "_pw_obj": p,
        })
    return devices


def _pick_device(devices: List[Dict[str, Any]], preferred_id: str = "") -> Optional[Dict[str, Any]]:
    if not devices:
        return None
    if preferred_id:
        for d in devices:
            if d["id"] == preferred_id:
                return d
    rc, def_sink, _ = btpw._run_cmd(["pactl", "get-default-sink"])
    if rc == 0:
        ds = def_sink.strip()
        for d in devices:
            if d["kind"] != "usb_hid" and (d.get("sink_name") == ds or ds == btpw.PW_SINK_NAME):
                return d
    return devices[0]


def _serialize_bands(bands: Sequence[FilterBand]) -> List[dict]:
    return [vars(b.clamped()) for b in bands]


def _parse_bands_json(raw: str) -> List[FilterBand]:
    items = json.loads(raw)
    return [FilterBand(**x).clamped() for x in items]


def _public_device_dict(d: Dict[str, Any]) -> Dict[str, Any]:
    return {k: v for k, v in d.items() if not k.startswith("_")}


def _entry_to_dict(e: db.AutoEqEntry) -> Dict[str, Any]:
    return {
        "name": e.name,
        "source": e.source_rig,
        "rel_path": e.unique_key,
        "raw_rel_path": e.rel_path,
        "form_factor": e.form_factor,
        "label": f"{e.name} [{e.source_rig}]",
    }


def _find_entry_by_key(entries: List[db.AutoEqEntry], key: str) -> Optional[db.AutoEqEntry]:
    for e in entries:
        if e.unique_key == key or e.rel_path == key:
            return e
    return None


def _downsample_curve(freqs: np.ndarray, vals: np.ndarray, points: int = 64) -> List[List[float]]:
    grid = np.geomspace(20.0, 20000.0, points)
    interp = np.interp(np.log10(grid), np.log10(freqs), vals)
    return [[round(float(f), 1), round(float(v), 2)] for f, v in zip(grid, interp)]


def cmd_status(preferred_id: str = "", refresh_db: bool = False) -> Dict[str, Any]:
    devices = _discover_all_devices()
    active = _pick_device(devices, preferred_id)

    bands: List[FilterBand] = _flat_bands(active["max_bands"] if active else DEFAULT_VIRTUAL_BANDS)
    dsp_enabled = True
    read_error: Optional[str] = None
    pw_active = btpw.is_pipewire_eq_running()

    if active:
        if active["kind"] == "usb_hid":
            try:
                _, dsp_enabled, bands = chu2_hid.read_chu2_eq(active["_usb_obj"])
            except Exception as exc:
                read_error = str(exc)
                bands = _flat_bands(active["max_bands"])
        else:
            pw_obj: btpw.PipeWireAudioDevice = active["_pw_obj"]
            bands = btpw.load_bt_saved_bands(pw_obj.mac or pw_obj.sink_name or pw_obj.display_name, pw_obj.max_bands)
            dsp_enabled = pw_active

    entries: List[db.AutoEqEntry] = []
    active_sources = 0
    total_sources = 1 + len(db.SQUIG_SOURCES)
    db_error: Optional[str] = None
    auto_src_entry: Optional[db.AutoEqEntry] = None
    stock_fr_curve: List[List[float]] = []

    try:
        entries, active_sources, total_sources = db.load_catalog(force_refresh=refresh_db)
        hint = active["autoeq_hint"] if (active and active.get("autoeq_hint")) else ""
        auto_src_entry = db.find_best_entry_any_brand(entries, hint) if hint else None
        if auto_src_entry is not None:
            try:
                f_fr, val_fr = db.reconstruct_fr_curve(auto_src_entry)
                stock_fr_curve = _downsample_curve(f_fr, val_fr)
            except Exception:
                pass
    except Exception as exc:
        db_error = str(exc)

    # Provide initial source suggestions (matched entry first + top Moondrop/popular entries for back-compat ComboBox)
    quick_sources: List[db.AutoEqEntry] = []
    seen_keys = set()
    if auto_src_entry is not None:
        quick_sources.append(auto_src_entry)
        seen_keys.add(auto_src_entry.unique_key)
    for e in entries:
        if len(quick_sources) >= 80:
            break
        if e.unique_key not in seen_keys:
            quick_sources.append(e)
            seen_keys.add(e.unique_key)

    return {
        "ok": True,
        "device": _public_device_dict(active) if active else None,
        "devices": [_public_device_dict(d) for d in devices],
        "bands": _serialize_bands(bands),
        "dsp_enabled": dsp_enabled,
        "pipewire_active": pw_active,
        "read_error": read_error,
        "presets": _load_presets(),
        "targets": [{"label": k, "file": v or ""} for k, v in db.CURATED_TARGETS.items()],
        "moondrop_models": [_entry_to_dict(e) for e in quick_sources],
        "auto_source_index": 0,
        "auto_source_entry": _entry_to_dict(auto_src_entry) if auto_src_entry else None,
        "stock_fr_curve": stock_fr_curve,
        "total_db_count": len(entries),
        "active_sources": active_sources,
        "total_sources": total_sources,
        "db_error": db_error,
    }


def cmd_apply(device_id: str, bands_json: str, commit: bool = False, enable_eq: bool = True) -> Dict[str, Any]:
    bands = _parse_bands_json(bands_json)
    devices = _discover_all_devices()
    dev = _pick_device(devices, device_id)
    if not dev:
        return {"ok": False, "error": "No hay ningún dispositivo de audio conectado."}

    fitted = _fit_band_count(bands, dev["max_bands"])
    if dev["kind"] == "usb_hid":
        chu2_hid.write_chu2_eq(fitted, commit=commit, enable_eq=enable_eq, info=dev["_usb_obj"])
        return {
            "ok": True,
            "kind": "usb_hid",
            "dsp_enabled": enable_eq,
            "committed": commit,
            "bands": _serialize_bands(fitted),
            "message": (
                f"Guardado en flash ({dev['display_name']})."
                if commit
                else (f"DSP {'activo' if enable_eq else 'en bypass'} ({dev['display_name']}).")
            ),
        }

    pw_obj: btpw.PipeWireAudioDevice = dev["_pw_obj"]
    active = btpw.apply_pipewire_eq(pw_obj, fitted, enable_eq=enable_eq)
    return {
        "ok": True,
        "kind": dev["kind"],
        "dsp_enabled": active,
        "pipewire_active": active,
        "bands": _serialize_bands(fitted),
        "message": (
            f"EQ PipeWire {dev['max_bands']}B activo para {dev['display_name']}."
            if active
            else f"EQ en bypass/plano para {dev['display_name']}."
        ),
    }


def cmd_flat(device_id: str, commit: bool = False) -> Dict[str, Any]:
    devices = _discover_all_devices()
    dev = _pick_device(devices, device_id)
    n = dev["max_bands"] if dev else DEFAULT_VIRTUAL_BANDS
    bands = _flat_bands(n)
    if not dev:
        btpw.stop_pipewire_eq()
        return {"ok": True, "bands": _serialize_bands(bands), "pipewire_active": False, "message": "EQ en plano (0 dB)."}
    return cmd_apply(dev["id"], json.dumps(_serialize_bands(bands)), commit=commit, enable_eq=True)


def cmd_search(query: str, moondrop_only: bool = False, in_ear_only: bool = False, limit: int = 80) -> Dict[str, Any]:
    entries = db.load_index(force_refresh=False)
    norm_q = db.normalize_search_text(query)
    tokens = [t for t in norm_q.split() if t]
    matches: List[db.AutoEqEntry] = []
    for e in entries:
        if in_ear_only and e.form_factor != "in-ear":
            continue
        if moondrop_only and "moondrop" not in e.name.lower():
            continue
        hay = db.normalize_search_text(f"{e.name} {e.source_rig}")
        if all(tok in hay for tok in tokens):
            matches.append(e)

    def _rank(e: db.AutoEqEntry) -> tuple:
        s = e.source_rig.lower()
        name_norm = db.normalize_search_text(e.name)
        exact_boost = 0 if (norm_q and norm_q in name_norm) else 1
        if "super* review" in s:
            r = 0
        elif "crinacle on 711" in s:
            r = 1
        elif "regancipher" in s:
            r = 2
        elif "oratory1990" in s:
            r = 3
        elif "aftersound" in s or "hbb" in s:
            r = 4
        elif e.form_factor == "in-ear":
            r = 5
        else:
            r = 6
        return (exact_boost, r, len(e.name), e.name.lower())

    matches.sort(key=_rank)
    return {
        "ok": True,
        "total_matches": len(matches),
        "results": [_entry_to_dict(e) for e in matches[:limit]],
    }


def cmd_source_fr(src_key: str) -> Dict[str, Any]:
    entries = db.load_index(force_refresh=False)
    src = _find_entry_by_key(entries, src_key)
    if not src:
        return {"ok": False, "error": "Modelo origen no encontrado."}
    f_fr, val_fr = db.reconstruct_fr_curve(src)
    return {"ok": True, "entry": _entry_to_dict(src), "stock_fr_curve": _downsample_curve(f_fr, val_fr)}


def cmd_autoeq_match(
    src_key: str, tgt_key: str, max_freq: float, max_bands: int, max_gain: float, supports_shelves: bool
) -> Dict[str, Any]:
    entries = db.load_index(force_refresh=False)
    src = _find_entry_by_key(entries, src_key)
    tgt = _find_entry_by_key(entries, tgt_key)
    if not src or not tgt:
        return {"ok": False, "error": "Modelo origen o destino no encontrado en el catálogo."}
    freqs, delta_db, bands, rmse, warn = db.compute_match_headphone_peq(
        src,
        tgt,
        max_freq_fit=max_freq,
        max_bands=max_bands,
        max_gain_db=max_gain,
        supports_shelves=supports_shelves,
    )
    f_s, fr_s = db.reconstruct_fr_curve(src)
    f_t, fr_t = db.reconstruct_fr_curve(tgt)
    mask_s = (f_s >= 200.0) & (f_s <= 3000.0)
    fr_t_on_s = np.interp(np.log10(f_s), np.log10(f_t), fr_t)
    delta_on_s = np.interp(np.log10(f_s), np.log10(freqs), delta_db)
    shift = float(np.mean((fr_s[mask_s] + delta_on_s[mask_s]) - fr_t_on_s[mask_s]))
    fr_t_aligned = fr_t_on_s + shift

    return {
        "ok": True,
        "bands": _serialize_bands(bands),
        "rmse": round(rmse, 2),
        "warning": warn,
        "target_curve": _downsample_curve(freqs, delta_db),
        "stock_fr_curve": _downsample_curve(f_s, fr_s),
        "target_fr_curve": _downsample_curve(f_s, fr_t_aligned),
        "target_label": tgt.name,
        "message": f"{src.name} → {tgt.name} (RMSE {rmse:.2f} dB)",
    }


def cmd_autoeq_target(
    src_key: str, target_csv: str, max_freq: float, max_bands: int, max_gain: float, supports_shelves: bool
) -> Dict[str, Any]:
    entries = db.load_index(force_refresh=False)
    src = _find_entry_by_key(entries, src_key)
    if not src:
        return {"ok": False, "error": "Modelo origen no encontrado en el catálogo."}
    csv_arg: Optional[str] = target_csv if target_csv else None
    freqs, target_db, bands, rmse = db.compute_target_curve_peq(
        src,
        csv_arg,
        max_freq_fit=max_freq,
        max_bands=max_bands,
        max_gain_db=max_gain,
        supports_shelves=supports_shelves,
    )
    f_s, fr_s = db.reconstruct_fr_curve(src)
    f_t, fr_t = db.reconstruct_target_csv_fr(csv_arg, src.form_factor)
    mask_s = (f_s >= 200.0) & (f_s <= 3000.0)
    fr_t_on_s = np.interp(np.log10(f_s), np.log10(f_t), fr_t)
    delta_on_s = np.interp(np.log10(f_s), np.log10(freqs), target_db)
    shift = float(np.mean((fr_s[mask_s] + delta_on_s[mask_s]) - fr_t_on_s[mask_s]))
    fr_t_aligned = fr_t_on_s + shift

    return {
        "ok": True,
        "bands": _serialize_bands(bands),
        "rmse": round(rmse, 2),
        "target_curve": _downsample_curve(freqs, target_db),
        "stock_fr_curve": _downsample_curve(f_s, fr_s),
        "target_fr_curve": _downsample_curve(f_s, fr_t_aligned),
        "target_label": target_csv.replace(".csv", "") if target_csv else "AutoEq Target",
        "message": f"{src.name} → Target (RMSE {rmse:.2f} dB)",
    }


def cmd_bt_spp(mac: str, action: str, preset_idx: int = 0, bands_json: str = "[]") -> Dict[str, Any]:
    if action == "preset":
        btpw.bt_spp_set_preset(mac, preset_idx)
        return {"ok": True, "message": f"Preset Bluetooth SPP #{preset_idx} enviado."}
    if action == "read":
        bands = btpw.bt_spp_read_eq(mac)
        return {"ok": True, "bands": _serialize_bands(bands), "message": "Leído EQ desde firmware Bluetooth."}
    if action == "write":
        bands = _parse_bands_json(bands_json)
        btpw.bt_spp_write_eq(mac, bands)
        return {"ok": True, "message": "EQ guardado en firmware Bluetooth SPP."}
    return {"ok": False, "error": f"Acción SPP desconocida: {action}"}


def _b64(val: Optional[str]) -> str:
    if not val:
        return ""
    return base64.b64decode(val.encode("ascii")).decode("utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_st = sub.add_parser("status")
    p_st.add_argument("--device-id", default="")
    p_st.add_argument("--refresh-db", action="store_true")

    p_sf = sub.add_parser("source-fr")
    p_sf.add_argument("--src", default="")
    p_sf.add_argument("--src-b64", default="")

    p_ap = sub.add_parser("apply")
    p_ap.add_argument("--device-id", default="")
    p_ap.add_argument("--bands-json", default="")
    p_ap.add_argument("--bands-b64", default="")
    p_ap.add_argument("--commit", action="store_true")
    p_ap.add_argument("--bypass", action="store_true")

    p_fl = sub.add_parser("flat")
    p_fl.add_argument("--device-id", default="")
    p_fl.add_argument("--commit", action="store_true")

    p_se = sub.add_parser("search")
    p_se.add_argument("--q", default="")
    p_se.add_argument("--q-b64", default="")
    p_se.add_argument("--moondrop-only", action="store_true")
    p_se.add_argument("--in-ear-only", action="store_true")
    p_se.add_argument("--limit", type=int, default=80)

    p_am = sub.add_parser("autoeq-match")
    p_am.add_argument("--src", default="")
    p_am.add_argument("--src-b64", default="")
    p_am.add_argument("--tgt", default="")
    p_am.add_argument("--tgt-b64", default="")
    p_am.add_argument("--max-freq", type=float, default=10000.0)
    p_am.add_argument("--bands", type=int, default=DEFAULT_VIRTUAL_BANDS)
    p_am.add_argument("--max-gain", type=float, default=12.0)
    p_am.add_argument("--no-shelves", action="store_true")

    p_at = sub.add_parser("autoeq-target")
    p_at.add_argument("--src", default="")
    p_at.add_argument("--src-b64", default="")
    p_at.add_argument("--target", default="")
    p_at.add_argument("--target-b64", default="")
    p_at.add_argument("--max-freq", type=float, default=10000.0)
    p_at.add_argument("--bands", type=int, default=DEFAULT_VIRTUAL_BANDS)
    p_at.add_argument("--max-gain", type=float, default=12.0)
    p_at.add_argument("--no-shelves", action="store_true")

    p_ps = sub.add_parser("preset-save")
    p_ps.add_argument("--name", default="")
    p_ps.add_argument("--name-b64", default="")
    p_ps.add_argument("--bands-json", default="")
    p_ps.add_argument("--bands-b64", default="")

    p_pd = sub.add_parser("preset-delete")
    p_pd.add_argument("--name", default="")
    p_pd.add_argument("--name-b64", default="")

    p_im = sub.add_parser("import-apo")
    p_im.add_argument("--text-b64", required=True)
    p_im.add_argument("--bands", type=int, default=DEFAULT_VIRTUAL_BANDS)

    p_ex = sub.add_parser("export-apo")
    p_ex.add_argument("--bands-b64", required=True)

    p_spp = sub.add_parser("bt-spp")
    p_spp.add_argument("--mac", required=True)
    p_spp.add_argument("--action", choices=["preset", "read", "write"], required=True)
    p_spp.add_argument("--preset-idx", type=int, default=0)
    p_spp.add_argument("--bands-json", default="[]")
    p_spp.add_argument("--bands-b64", default="")

    args = parser.parse_args()
    try:
        if args.cmd == "status":
            res = cmd_status(args.device_id, args.refresh_db)
        elif args.cmd == "source-fr":
            src = _b64(args.src_b64) if args.src_b64 else args.src
            res = cmd_source_fr(src)
        elif args.cmd == "apply":
            bj = _b64(args.bands_b64) if args.bands_b64 else args.bands_json
            res = cmd_apply(args.device_id, bj, args.commit, enable_eq=not args.bypass)
        elif args.cmd == "flat":
            res = cmd_flat(args.device_id, args.commit)
        elif args.cmd == "search":
            q = _b64(args.q_b64) if args.q_b64 else args.q
            res = cmd_search(q, args.moondrop_only, args.in_ear_only, args.limit)
        elif args.cmd == "autoeq-match":
            src = _b64(args.src_b64) if args.src_b64 else args.src
            tgt = _b64(args.tgt_b64) if args.tgt_b64 else args.tgt
            res = cmd_autoeq_match(src, tgt, args.max_freq, args.bands, args.max_gain, not args.no_shelves)
        elif args.cmd == "autoeq-target":
            src = _b64(args.src_b64) if args.src_b64 else args.src
            tgt = _b64(args.target_b64) if args.target_b64 else args.target
            res = cmd_autoeq_target(src, tgt, args.max_freq, args.bands, args.max_gain, not args.no_shelves)
        elif args.cmd == "preset-save":
            nm = _b64(args.name_b64) if args.name_b64 else args.name
            bj = _b64(args.bands_b64) if args.bands_b64 else args.bands_json
            presets = _load_presets()
            presets[nm] = _serialize_bands(_parse_bands_json(bj))
            _save_presets(presets)
            res = {"ok": True, "presets": presets, "message": f"Preset '{nm}' guardado."}
        elif args.cmd == "preset-delete":
            nm = _b64(args.name_b64) if args.name_b64 else args.name
            presets = _load_presets()
            if nm in presets:
                del presets[nm]
                _save_presets(presets)
            res = {"ok": True, "presets": presets, "message": f"Preset '{nm}' eliminado."}
        elif args.cmd == "import-apo":
            parsed = db.parse_parametric_eq(_b64(args.text_b64))
            if not parsed:
                res = {"ok": False, "error": "No se encontraron líneas 'Filter N: ON PK/LSC/HSC ...' válidas."}
            else:
                fitted = _fit_band_count(parsed, args.bands)
                res = {"ok": True, "bands": _serialize_bands(fitted), "message": f"Importadas {min(len(parsed), args.bands)} bandas APO."}
        elif args.cmd == "export-apo":
            bands = _parse_bands_json(_b64(args.bands_b64))
            res = {"ok": True, "text": db.export_parametric_eq(bands)}
        elif args.cmd == "bt-spp":
            bj = _b64(args.bands_b64) if args.bands_b64 else args.bands_json
            res = cmd_bt_spp(args.mac, args.action, args.preset_idx, bj)
        else:
            res = {"ok": False, "error": "Comando no soportado"}
    except Exception as exc:
        res = {"ok": False, "error": str(exc)}

    print(json.dumps(res, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
