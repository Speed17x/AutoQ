#!/usr/bin/env python3
"""Windows Beta JSON CLI backend for AutoQ [BETA — UNTESTED].

Provides contract parity with `moondrop_backend.py` (Linux) using:
  - `chu2_hid_win.py` (hidapi USB-C HID direct DSP)
  - `win_eq_apo.py` (Equalizer APO 10-band system-wide PEQ)
  - `autoeq_db.py` (8 open AutoEq + Squiglink databases)
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
import chu2_hid_win as winhid
import win_eq_apo as apo

DEFAULT_VIRTUAL_BANDS = 10
CONFIG_DIR = Path(os.environ.get("LOCALAPPDATA", Path.home() / ".config")) / "AutoQ"
PRESETS_FILE = CONFIG_DIR / "presets.json"
STATE_FILE = CONFIG_DIR / "win_eq_state.json"


def _b64(val: Optional[str]) -> str:
    if not val:
        return ""
    return base64.b64decode(val.encode("ascii")).decode("utf-8")


def _serialize_bands(bands: Sequence[FilterBand]) -> List[Dict[str, Any]]:
    return [
        {
            "type": b.type,
            "frequency": round(float(b.frequency), 1),
            "gain": round(float(b.gain), 2),
            "q": round(float(b.q), 3),
        }
        for b in bands
    ]


def _parse_bands_json(raw: str) -> List[FilterBand]:
    items = json.loads(raw) if raw else []
    out: List[FilterBand] = []
    for it in items:
        out.append(
            FilterBand(
                type=it.get("type", "peaking"),
                frequency=float(it.get("frequency", 1000.0)),
                gain=float(it.get("gain", 0.0)),
                q=float(it.get("q", 1.0)),
            ).clamped()
        )
    return out


def _fit_band_count(bands: Sequence[FilterBand], n: int) -> List[FilterBand]:
    out = [b.clamped() for b in list(bands)[:n]]
    defaults = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
    while len(out) < n:
        f = float(defaults[len(out) % len(defaults)])
        out.append(FilterBand("peaking", f, 0.0, 1.0))
    return out


def _load_presets() -> Dict[str, List[Dict[str, Any]]]:
    if PRESETS_FILE.exists():
        try:
            data = json.loads(PRESETS_FILE.read_text(encoding="utf-8"))
            if isinstance(data, dict):
                return data
        except Exception:
            pass
    return {
        "Plano (0 dB)": _serialize_bands(_fit_band_count([], DEFAULT_VIRTUAL_BANDS)),
    }


def _save_presets(presets: Dict[str, List[Dict[str, Any]]]) -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    PRESETS_FILE.write_text(json.dumps(presets, indent=2, ensure_ascii=False), encoding="utf-8")


def _load_virtual_state() -> Dict[str, Any]:
    if STATE_FILE.exists():
        try:
            return json.loads(STATE_FILE.read_text(encoding="utf-8"))
        except Exception:
            pass
    return {"enabled": True, "bands": _serialize_bands(_fit_band_count([], DEFAULT_VIRTUAL_BANDS))}


def _save_virtual_state(enabled: bool, bands: Sequence[FilterBand]) -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(
        json.dumps({"enabled": enabled, "bands": _serialize_bands(bands)}, indent=2, ensure_ascii=False),
        encoding="utf-8",
    )


def _list_devices() -> List[Dict[str, Any]]:
    devices: List[Dict[str, Any]] = []
    for d in winhid.detect_devices():
        devices.append(
            {
                "id": f"usb:{d.hidraw_path}",
                "transport": "usb_hid",
                "transport_label": "USB DSP",
                "display_name": d.display_name,
                "protocol": d.protocol,
                "max_bands": d.max_bands,
                "max_gain_db": d.max_gain_db,
                "supports_shelves": d.supports_shelves,
                "disconnects_on_commit": d.disconnects_on_commit,
                "autoeq_hint": d.autoeq_hint,
                "hidraw_path": d.hidraw_path,
                "vid_pid": d.vid_pid,
            }
        )

    apo_dir = apo.find_apo_config_dir()
    devices.append(
        {
            "id": "win:equalizer_apo",
            "transport": "win_apo",
            "transport_label": "EQ APO" if apo_dir else "APO (NO INSTALADO)",
            "display_name": "Windows Global (Equalizer APO)" if apo_dir else "Equalizer APO (no detectado)",
            "protocol": "equalizer_apo",
            "max_bands": DEFAULT_VIRTUAL_BANDS,
            "max_gain_db": 15.0,
            "supports_shelves": True,
            "disconnects_on_commit": False,
            "autoeq_hint": "",
            "apo_ready": apo_dir is not None,
        }
    )
    return devices


def _pick_device(devices: List[Dict[str, Any]], device_id: str) -> Optional[Dict[str, Any]]:
    if device_id:
        for d in devices:
            if d["id"] == device_id:
                return d
    return devices[0] if devices else None


def cmd_status(device_id: str, refresh_db: bool) -> Dict[str, Any]:
    entries = db.fetch_index(force_refresh=refresh_db)
    devices = _list_devices()
    active = _pick_device(devices, device_id)

    dsp_enabled = True
    bands: List[Dict[str, Any]] = []
    warning = "Versión Windows BETA (sin testear en hardware real)."

    if active and active["transport"] == "usb_hid":
        hid_devs = {f"usb:{d.hidraw_path}": d for d in winhid.detect_devices()}
        info = hid_devs.get(active["id"])
        if info is not None:
            try:
                with winhid.MoondropWinHidDevice(info) as dev:
                    dsp_enabled, read_bands = dev.read_eq()
                    bands = _serialize_bands(read_bands)
            except Exception as exc:
                warning = f"Windows BETA — Error leyendo HID: {exc}"
                bands = _serialize_bands(_fit_band_count([], active["max_bands"]))
    else:
        st = _load_virtual_state()
        dsp_enabled = bool(st.get("enabled", True))
        bands = st.get("bands", _serialize_bands(_fit_band_count([], DEFAULT_VIRTUAL_BANDS)))

    return {
        "ok": True,
        "platform": "windows_beta",
        "device": active,
        "devices": devices,
        "dsp_enabled": dsp_enabled,
        "bands": bands,
        "presets": _load_presets(),
        "targets": list(db.CURATED_TARGETS.keys()),
        "total_db_count": len(entries),
        "active_sources": 8,
        "total_sources": 8,
        "warning": warning,
    }


def cmd_apply(device_id: str, bands_json: str, commit: bool, enable_eq: bool) -> Dict[str, Any]:
    devices = _list_devices()
    active = _pick_device(devices, device_id)
    if not active:
        return {"ok": False, "error": "No hay dispositivo disponible."}
    bands = _fit_band_count(_parse_bands_json(bands_json), int(active["max_bands"]))

    if active["transport"] == "usb_hid":
        hid_devs = {f"usb:{d.hidraw_path}": d for d in winhid.detect_devices()}
        info = hid_devs.get(active["id"])
        if info is None:
            return {"ok": False, "error": "Dispositivo USB HID desconectado."}
        with winhid.MoondropWinHidDevice(info) as dev:
            dev.write_eq(bands, commit=commit, enable_eq=enable_eq)
        msg = "Guardado en Flash USB HID." if commit else ("EQ aplicado en RAM USB HID." if enable_eq else "Bypass USB HID activado.")
        return {"ok": True, "bands": _serialize_bands(bands), "dsp_enabled": enable_eq, "message": msg}

    _save_virtual_state(enable_eq, bands)
    res = apo.apply_apo_eq(bands, enabled=enable_eq)
    if not res.get("ok"):
        return res
    return {"ok": True, "bands": _serialize_bands(bands), "dsp_enabled": enable_eq, "message": res.get("message", "OK")}


def cmd_flat(device_id: str, commit: bool) -> Dict[str, Any]:
    devices = _list_devices()
    active = _pick_device(devices, device_id)
    n = int(active["max_bands"]) if active else DEFAULT_VIRTUAL_BANDS
    flat_bands = _fit_band_count([], n)
    return cmd_apply(device_id, json.dumps(_serialize_bands(flat_bands)), commit=commit, enable_eq=True)


def cmd_search(q: str, moondrop_only: bool, in_ear_only: bool, limit: int) -> Dict[str, Any]:
    entries = db.fetch_index(force_refresh=False)
    matches = db.search_entries(entries, q, moondrop_only=moondrop_only, in_ear_only=in_ear_only, limit=limit)
    return {
        "ok": True,
        "results": [
            {
                "name": e.name,
                "rel_path": e.rel_path,
                "source_rig": e.source_rig,
                "source_kind": e.source_kind,
                "form_factor": e.form_factor,
            }
            for e in matches
        ],
    }


def _find_entry(rel_path: str) -> Optional[db.AutoEqEntry]:
    for e in db.fetch_index(force_refresh=False):
        if e.rel_path == rel_path:
            return e
    return None


def cmd_source_fr(src_path: str) -> Dict[str, Any]:
    entry = _find_entry(src_path)
    if entry is None:
        return {"ok": False, "error": f"No se encontró la curva fuente: {src_path}"}
    fr = db.fetch_fr_curve(entry)
    pts = [[round(float(f), 1), round(float(v), 2)] for f, v in zip(db.STANDARD_GRID, fr)]
    return {"ok": True, "source_fr": pts, "name": entry.name}


def cmd_autoeq_target(src: str, target_name: str, max_freq: float, n_bands: int, max_gain: float, shelves: bool) -> Dict[str, Any]:
    entry = _find_entry(src)
    if entry is None:
        return {"ok": False, "error": f"Curva fuente no encontrada: {src}"}
    fitted, rmse, src_fr, tgt_fr = db.match_to_target(
        entry, target_name, n_bands=n_bands, max_freq=max_freq, max_gain=max_gain, allow_shelves=shelves
    )
    return {
        "ok": True,
        "bands": _serialize_bands(fitted),
        "rmse": round(float(rmse), 2),
        "source_fr": [[round(float(f), 1), round(float(v), 2)] for f, v in zip(db.STANDARD_GRID, src_fr)],
        "target_fr": [[round(float(f), 1), round(float(v), 2)] for f, v in zip(db.STANDARD_GRID, tgt_fr)],
        "message": f"Ajuste a '{target_name}' listo (RMSE {rmse:.2f} dB).",
    }


def cmd_autoeq_match(src: str, tgt: str, max_freq: float, n_bands: int, max_gain: float, shelves: bool) -> Dict[str, Any]:
    e_src = _find_entry(src)
    e_tgt = _find_entry(tgt)
    if e_src is None or e_tgt is None:
        return {"ok": False, "error": "No se encontró la curva fuente o la curva objetivo."}
    fitted, rmse, src_fr, tgt_fr = db.match_headphone_to_headphone(
        e_src, e_tgt, n_bands=n_bands, max_freq=max_freq, max_gain=max_gain, allow_shelves=shelves
    )
    return {
        "ok": True,
        "bands": _serialize_bands(fitted),
        "rmse": round(float(rmse), 2),
        "source_fr": [[round(float(f), 1), round(float(v), 2)] for f, v in zip(db.STANDARD_GRID, src_fr)],
        "target_fr": [[round(float(f), 1), round(float(v), 2)] for f, v in zip(db.STANDARD_GRID, tgt_fr)],
        "message": f"Imitación de '{e_tgt.name}' lista (RMSE {rmse:.2f} dB).",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="AutoQ Windows Beta Backend")
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
            res = {"ok": False, "error": "Bluetooth SPP aún no está implementado en la versión Windows Beta."}
        else:
            res = {"ok": False, "error": "Comando no soportado"}
    except Exception as exc:
        res = {"ok": False, "error": str(exc)}

    print(json.dumps(res, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
