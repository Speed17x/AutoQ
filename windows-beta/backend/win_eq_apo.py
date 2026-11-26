"""Windows system-wide 10-band EQ backend via Equalizer APO [BETA — UNTESTED].

Writes `autoq_eq.txt` inside Equalizer APO's `config/` directory and ensures
`Include: autoq_eq.txt` is present in `config.txt`.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Dict, List, Optional, Sequence

import numpy as np

from biquad import FilterBand, combined_response_db

DEFAULT_APO_PATHS = [
    Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "EqualizerAPO" / "config",
    Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "EqualizerAPO" / "config",
]
AUTOQ_APO_FILENAME = "autoq_eq.txt"


def find_apo_config_dir() -> Optional[Path]:
    override = os.environ.get("AUTOQ_APO_CONFIG_DIR", "").strip()
    if override:
        p = Path(override)
        if p.is_dir():
            return p
    for candidate in DEFAULT_APO_PATHS:
        if candidate.is_dir():
            return candidate
    return None


def compute_safe_preamp_db(bands: Sequence[FilterBand]) -> float:
    if not bands:
        return 0.0
    grid = np.geomspace(20.0, 20000.0, 256)
    resp = combined_response_db(bands, grid)
    peak = float(np.max(resp))
    return round(-peak - 0.2, 2) if peak > 0.0 else 0.0


def format_apo_config(bands: Sequence[FilterBand], enabled: bool = True) -> str:
    if not enabled:
        return "# AutoQ — Bypass activo\nPreamp: 0.0 dB\n"
    preamp = compute_safe_preamp_db(bands)
    lines = [
        "# Generado automáticamente por AutoQ (Windows Beta)",
        f"Preamp: {preamp:.2f} dB",
    ]
    type_map = {"peaking": "PK", "low_shelf": "LSC", "high_shelf": "HSC"}
    for idx, b in enumerate(bands, start=1):
        code = type_map.get(b.type, "PK")
        lines.append(f"Filter {idx}: ON {code} Fc {b.frequency:.1f} Hz Gain {b.gain:.2f} dB Q {b.q:.3f}")
    return "\n".join(lines) + "\n"


def apply_apo_eq(bands: Sequence[FilterBand], enabled: bool = True) -> Dict[str, object]:
    cfg_dir = find_apo_config_dir()
    if cfg_dir is None:
        return {
            "ok": False,
            "error": (
                "No se encontró Equalizer APO en 'C:\\Program Files\\EqualizerAPO\\config'. "
                "Instala Equalizer APO para usar EQ global de 10 bandas en Windows."
            ),
        }
    target_file = cfg_dir / AUTOQ_APO_FILENAME
    main_cfg = cfg_dir / "config.txt"
    try:
        target_file.write_text(format_apo_config(bands, enabled=enabled), encoding="utf-8")
        include_line = f"Include: {AUTOQ_APO_FILENAME}"
        if main_cfg.exists():
            txt = main_cfg.read_text(encoding="utf-8", errors="ignore")
            if include_line not in txt:
                sep = "" if txt.endswith("\n") or not txt else "\n"
                main_cfg.write_text(txt + sep + include_line + "\n", encoding="utf-8")
        else:
            main_cfg.write_text(include_line + "\n", encoding="utf-8")
    except PermissionError as exc:
        return {
            "ok": False,
            "error": f"Sin permisos para escribir en {cfg_dir}. Ejecuta AutoQ como Administrador o da permisos a la carpeta config.",
        }
    except OSError as exc:
        return {"ok": False, "error": f"Error escribiendo Equalizer APO: {exc}"}

    return {
        "ok": True,
        "path": str(target_file),
        "enabled": enabled,
        "message": f"EQ aplicado en Equalizer APO ({target_file.name})." if enabled else "Bypass aplicado en Equalizer APO.",
    }
