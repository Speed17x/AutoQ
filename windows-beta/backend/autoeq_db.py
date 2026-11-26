"""Multi-source AutoEq + Squiglink database client, cache, target loader, and PEQ matcher.

Sources (8 verified open repositories fetched in parallel):
  1. AutoEq GitHub (jaakkopasanen/AutoEq)
  2. Super* Review (Squiglink 711) — includes 7Hz x Crinacle Zero 3, Moondrop Golden Ages 2, etc.
  3. ReganCipher (Squiglink TWS/711) — includes Golden Ages 2 ANC, Space Travel 2, TWS catalog
  4. Aftersound (Squiglink 711)
  5. HBB (Squiglink 711)
  6. Paul Wasabii (Squiglink 711)
  7. Jaytiss (Squiglink 711)
  8. GadgetryTech (Squiglink Over-Ear / Headsets)
"""

from __future__ import annotations

import concurrent.futures
import csv
import dataclasses
import hashlib
import json
import os
import re
import urllib.parse
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np
import requests

from biquad import FilterBand, FilterType, combined_response_db, fit_peq_bands, _make_weights, _weighted_rmse

INDEX_URL = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/INDEX.md"
RESULTS_BASE_URL = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results"
TARGETS_BASE_URL = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/targets"

CACHE_DIR = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "chu2-dsp-widget"
STANDARD_GRID = np.geomspace(20.0, 20000.0, 128)

_INDEX_RE = re.compile(r"^- \[(.+?)\]\(\./(.+?)\) by (.+)$")
_PARAM_RE = re.compile(
    r"^Filter\s+\d+:\s+ON\s+(PK|LSC|HSC)\s+Fc\s+([\d.]+)\s+Hz\s+Gain\s+([+-]?[\d.]+)\s+dB\s+Q\s+([\d.]+)",
    re.IGNORECASE,
)

SQUIG_SOURCES: List[Dict[str, str]] = [
    {"id": "superreview", "label": "Super* Review (711)", "base": "https://squig.link/data/", "ff": "in-ear", "rig": "711"},
    {"id": "regancipher", "label": "ReganCipher (TWS/711)", "base": "https://regancipher.squig.link/data/", "ff": "in-ear", "rig": "711"},
    {"id": "aftersound", "label": "Aftersound (711)", "base": "https://aftersound.squig.link/data/", "ff": "in-ear", "rig": "711"},
    {"id": "hbb", "label": "HBB (711)", "base": "https://hbb.squig.link/data/", "ff": "in-ear", "rig": "711"},
    {"id": "paulwasabii", "label": "Paul Wasabii (711)", "base": "https://pw.squig.link/data/", "ff": "in-ear", "rig": "711"},
    {"id": "jaytiss", "label": "Jaytiss (711)", "base": "https://jaytiss.squig.link/data/", "ff": "in-ear", "rig": "711"},
    {"id": "gadgetrytech", "label": "GadgetryTech (Over-Ear)", "base": "https://gadgetrytech.squig.link/headsets/data/", "ff": "over-ear", "rig": "over-ear"},
]

CURATED_TARGETS: Dict[str, Optional[str]] = {
    "AutoEq In-Ear (Predeterminado)": None,
    "Harman In-Ear 2019": "Harman in-ear 2019.csv",
    "Harman In-Ear 2019 (sin boost bajos)": "Harman in-ear 2019 without bass.csv",
    "Harman In-Ear 2017-2": "Harman in-ear 2017-2.csv",
    "Harman Over-Ear 2018": "Harman over-ear 2018.csv",
    "JM-1 con filtros Harman": "JM-1 with Harman filters.csv",
    "Diffuse Field 5128 (-1 dB/oct)": "Diffuse field 5128 -1dB per octave.csv",
    "Oratory1990 In-Ear": "oratory1990 in-ear.csv",
}


def normalize_search_text(raw: str) -> str:
    """Normalize punctuation (e.g. Zero:3 -> zero 3) for tolerant cross-rig search."""
    return re.sub(r"\s+", " ", raw.lower().replace(":", " ").replace("-", " ").replace("_", " ")).strip()


@dataclasses.dataclass(frozen=True)
class AutoEqEntry:
    name: str
    rel_path: str
    source_rig: str
    source_kind: str = "autoeq"          # "autoeq" | "squiglink"
    squig_base_url: str = ""
    squig_file_stem: str = ""
    form_factor_override: str = ""
    rig_tag_override: str = ""

    @property
    def unique_key(self) -> str:
        prefix = "sq" if self.source_kind == "squiglink" else "ae"
        return f"{prefix}:{self.rel_path}"

    @property
    def form_factor(self) -> str:
        if self.form_factor_override:
            return self.form_factor_override
        low = urllib.parse.unquote(self.rel_path).lower()
        if "in-ear" in low:
            return "in-ear"
        if "over-ear" in low:
            return "over-ear"
        if "earbud" in low:
            return "earbud"
        return "in-ear"

    @property
    def rig_tag(self) -> str:
        if self.rig_tag_override:
            return self.rig_tag_override
        low = self.source_rig.lower()
        if "5128" in low:
            return "5128"
        if "4620" in low:
            return "4620"
        if "hms" in low:
            return "HMS"
        if any(k in low for k in ("711", "super* review", "super review", "oratory1990", "tonedeafmonk", "aftersound", "hbb", "paul wasabii", "jaytiss", "regancipher")):
            return "711"
        return self.source_rig

    @property
    def label(self) -> str:
        return f"{self.name} — {self.source_rig}"

    @property
    def folder_segment(self) -> str:
        return self.rel_path.rsplit("/", 1)[-1]

    @property
    def graphic_eq_url(self) -> str:
        return f"{RESULTS_BASE_URL}/{self.rel_path}/{self.folder_segment}%20GraphicEQ.txt"

    @property
    def parametric_eq_url(self) -> str:
        return f"{RESULTS_BASE_URL}/{self.rel_path}/{self.folder_segment}%20ParametricEQ.txt"


def _ensure_cache() -> None:
    (CACHE_DIR / "profiles").mkdir(parents=True, exist_ok=True)
    (CACHE_DIR / "targets").mkdir(parents=True, exist_ok=True)


def _fetch_text(url: str, cache_file: Path, force_refresh: bool = False, timeout: float = 12.0) -> str:
    _ensure_cache()
    if cache_file.exists() and cache_file.stat().st_size > 0 and not force_refresh:
        return cache_file.read_text(encoding="utf-8", errors="ignore")
    resp = requests.get(url, timeout=timeout, headers={"User-Agent": "AutoQ-Linux/2.0"})
    resp.raise_for_status()
    text = resp.text
    cache_file.write_text(text, encoding="utf-8")
    return text


def parse_squig_phone_book(raw_json: str, src: Dict[str, str]) -> List[AutoEqEntry]:
    try:
        data = json.loads(raw_json)
    except Exception:
        return []
    if not isinstance(data, list):
        return []

    entries: List[AutoEqEntry] = []
    for brand in data:
        if not isinstance(brand, dict):
            continue
        bname = str(brand.get("name", "")).strip()
        if bname.startswith("_"):
            continue
        phones = brand.get("phones")
        if not isinstance(phones, list):
            continue
        for ph in phones:
            if not isinstance(ph, dict):
                continue
            pname = str(ph.get("name", "")).strip()
            if not pname or pname.startswith("_"):
                continue
            full_base = f"{bname} {pname}".strip() if (bname and not pname.lower().startswith(bname.lower())) else pname
            file_val = ph.get("file")
            if isinstance(file_val, str):
                files = [file_val] if file_val.strip() else []
            elif isinstance(file_val, list):
                files = [str(x).strip() for x in file_val if str(x).strip()]
            else:
                files = []
            if not files:
                continue

            suf_val = ph.get("suffix")
            if isinstance(suf_val, list):
                suffixes = [str(x).strip() for x in suf_val]
            elif isinstance(suf_val, str):
                suffixes = [suf_val.strip()]
            else:
                suffixes = []

            for idx, stem in enumerate(files):
                suf = suffixes[idx] if idx < len(suffixes) else ""
                if suf:
                    variant = f"{full_base} {suf}".strip()
                elif len(files) > 1 and idx > 0:
                    variant = f"{full_base} (#{idx + 1})"
                else:
                    variant = full_base
                entries.append(
                    AutoEqEntry(
                        name=variant,
                        rel_path=f"{src['id']}/{stem}",
                        source_rig=src["label"],
                        source_kind="squiglink",
                        squig_base_url=src["base"],
                        squig_file_stem=stem,
                        form_factor_override=src["ff"],
                        rig_tag_override=src["rig"],
                    )
                )
    return entries


def parse_squig_fr_txt(text: str) -> Tuple[np.ndarray, np.ndarray]:
    """Parse REW / Squiglink frequency response text file -> sorted (freqs_hz, spl_db)."""
    pts: List[Tuple[float, float]] = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line[0] in ("*", "#", ";"):
            continue
        parts = re.split(r"[\s,;]+", line)
        if len(parts) < 2:
            continue
        try:
            f = float(parts[0])
            db_v = float(parts[1])
        except ValueError:
            continue
        if np.isfinite(f) and np.isfinite(db_v) and 10.0 <= f <= 25000.0:
            pts.append((f, db_v))
    if len(pts) < 10:
        raise ValueError("Archivo FR Squiglink inválido o vacío.")
    pts.sort(key=lambda x: x[0])

    out_f: List[float] = []
    out_db: List[float] = []
    i = 0
    n = len(pts)
    while i < n:
        f0 = pts[i][0]
        acc = pts[i][1]
        cnt = 1
        while i + 1 < n and abs(pts[i + 1][0] - f0) < 1e-6:
            i += 1
            acc += pts[i][1]
            cnt += 1
        out_f.append(f0)
        out_db.append(acc / cnt)
        i += 1
    return np.asarray(out_f, dtype=np.float64), np.asarray(out_db, dtype=np.float64)


def load_catalog(force_refresh: bool = False) -> Tuple[List[AutoEqEntry], int, int]:
    """Load merged catalog from AutoEq GitHub + 7 open Squiglink sources.

    Returns ``(entries, active_sources, total_sources)``.
    """
    _ensure_cache()
    total_sources = 1 + len(SQUIG_SOURCES)
    json_cache = CACHE_DIR / "catalog_v2.json"
    if json_cache.exists() and not force_refresh:
        try:
            raw = json.loads(json_cache.read_text(encoding="utf-8"))
            entries = [
                AutoEqEntry(
                    name=x[0],
                    rel_path=x[1],
                    source_rig=x[2],
                    source_kind=x[3],
                    squig_base_url=x[4],
                    squig_file_stem=x[5],
                    form_factor_override=x[6],
                    rig_tag_override=x[7],
                )
                for x in raw
            ]
            if entries:
                active = len({(e.source_kind, e.squig_base_url) for e in entries})
                return entries, min(total_sources, max(1, active)), total_sources
        except Exception:
            pass

    def _load_autoeq() -> List[AutoEqEntry]:
        try:
            text = _fetch_text(INDEX_URL, CACHE_DIR / "index.md", force_refresh=force_refresh, timeout=16.0)
            out: List[AutoEqEntry] = []
            for line in text.splitlines():
                m = _INDEX_RE.match(line.strip())
                if m:
                    out.append(AutoEqEntry(name=m.group(1), rel_path=m.group(2), source_rig=m.group(3)))
            return out
        except Exception:
            return []

    def _load_squig(src: Dict[str, str]) -> List[AutoEqEntry]:
        try:
            raw_txt = _fetch_text(
                f"{src['base']}phone_book.json",
                CACHE_DIR / f"squig_{src['id']}.json",
                force_refresh=force_refresh,
                timeout=10.0,
            )
            return parse_squig_phone_book(raw_txt, src)
        except Exception:
            return []

    combined: List[AutoEqEntry] = []
    ok_sources = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        f_ae = pool.submit(_load_autoeq)
        f_sqs = [pool.submit(_load_squig, src) for src in SQUIG_SOURCES]
        for fut in f_sqs:
            res = fut.result()
            if res:
                ok_sources += 1
                combined.extend(res)
        ae_res = f_ae.result()
        if ae_res:
            ok_sources += 1
            combined.extend(ae_res)

    if combined:
        json_cache.write_text(
            json.dumps(
                [
                    [
                        e.name,
                        e.rel_path,
                        e.source_rig,
                        e.source_kind,
                        e.squig_base_url,
                        e.squig_file_stem,
                        e.form_factor_override,
                        e.rig_tag_override,
                    ]
                    for e in combined
                ],
                ensure_ascii=False,
            ),
            encoding="utf-8",
        )
    return combined, ok_sources, total_sources


def load_index(force_refresh: bool = False) -> List[AutoEqEntry]:
    entries, _, _ = load_catalog(force_refresh=force_refresh)
    return entries


def find_moondrop_entries(entries: List[AutoEqEntry]) -> List[AutoEqEntry]:
    """Return Moondrop entries ranked by reliable rigs (back-compat helper)."""
    matches = [e for e in entries if "moondrop" in e.name.lower()]

    def _rank(e: AutoEqEntry) -> Tuple[int, str, str]:
        s = e.source_rig.lower()
        if "super* review" in s:
            r = 0
        elif "crinacle on 711" in s:
            r = 1
        elif "regancipher" in s:
            r = 2
        elif "oratory1990" in s:
            r = 3
        else:
            r = 4
        return (r, e.name.lower(), s)

    matches.sort(key=_rank)
    return matches


def find_best_entry_any_brand(entries: List[AutoEqEntry], hint: str) -> Optional[AutoEqEntry]:
    """Find best matching headphone/IEM/TWS across all AutoEq + Squiglink entries."""
    if not entries or not hint.strip():
        return None
    hint_norm = normalize_search_text(hint)
    tokens = [t for t in hint_norm.split() if len(t) >= 2]
    if not tokens:
        return None
    best: Optional[AutoEqEntry] = None
    best_score = 0
    for e in entries:
        low = normalize_search_text(e.name)
        matched = sum(1 for tok in tokens if tok in low)
        if matched == 0:
            continue
        score = matched * 14
        if matched == len(tokens):
            score += 18
        if low == hint_norm:
            score += 60
        elif low.startswith(hint_norm):
            score += 26
        elif hint_norm in low:
            score += 16
        rig = e.source_rig.lower()
        if "super* review" in rig:
            score += 7
        elif "oratory1990" in rig or "crinacle on 711" in rig:
            score += 6
        elif "regancipher" in rig:
            score += 5
        elif "aftersound" in rig:
            score += 4
        if score > best_score:
            best_score = score
            best = e
    return best


def find_best_moondrop_index(moondrop_entries: List[AutoEqEntry], hint: str) -> int:
    best = find_best_entry_any_brand(moondrop_entries, hint)
    if best is None:
        return 0 if moondrop_entries else -1
    for i, e in enumerate(moondrop_entries):
        if e.unique_key == best.unique_key:
            return i
    return 0


def fetch_graphic_eq(entry: AutoEqEntry, force_refresh: bool = False) -> Tuple[np.ndarray, np.ndarray]:
    digest = hashlib.sha1(entry.rel_path.encode("utf-8")).hexdigest()[:16]
    cache_path = CACHE_DIR / "profiles" / f"{digest}_graphic.txt"
    text = _fetch_text(entry.graphic_eq_url, cache_path, force_refresh=force_refresh)
    return parse_graphic_eq(text)


def fetch_parametric_eq(entry: AutoEqEntry, force_refresh: bool = False) -> List[FilterBand]:
    digest = hashlib.sha1(entry.rel_path.encode("utf-8")).hexdigest()[:16]
    cache_path = CACHE_DIR / "profiles" / f"{digest}_peq.txt"
    text = _fetch_text(entry.parametric_eq_url, cache_path, force_refresh=force_refresh)
    return parse_parametric_eq(text)


def fetch_target_csv(filename: str, force_refresh: bool = False) -> Tuple[np.ndarray, np.ndarray]:
    encoded = urllib.parse.quote(filename, safe="")
    url = f"{TARGETS_BASE_URL}/{encoded}"
    safe_name = hashlib.sha1(filename.encode("utf-8")).hexdigest()[:12] + ".csv"
    text = _fetch_text(url, CACHE_DIR / "targets" / safe_name, force_refresh=force_refresh)
    freqs: List[float] = []
    vals: List[float] = []
    reader = csv.reader(text.splitlines())
    next(reader, None)
    for row in reader:
        if len(row) >= 2:
            try:
                freqs.append(float(row[0]))
                vals.append(float(row[1]))
            except ValueError:
                continue
    if not freqs:
        raise ValueError(f"Target CSV vacío o inválido: {filename}")
    return np.asarray(freqs, dtype=np.float64), np.asarray(vals, dtype=np.float64)


def parse_graphic_eq(text: str) -> Tuple[np.ndarray, np.ndarray]:
    body = text.strip()
    if body.lower().startswith("graphiceq:"):
        body = body.split(":", 1)[1]
    freqs: List[float] = []
    gains: List[float] = []
    for pair in body.split(";"):
        pair = pair.strip()
        if not pair:
            continue
        parts = pair.split()
        if len(parts) >= 2:
            freqs.append(float(parts[0]))
            gains.append(float(parts[1]))
    if len(freqs) < 5:
        raise ValueError("Formato GraphicEQ inválido o incompleto.")
    return np.asarray(freqs, dtype=np.float64), np.asarray(gains, dtype=np.float64)


def parse_parametric_eq(text: str) -> List[FilterBand]:
    type_map: Dict[str, FilterType] = {"PK": "peaking", "LSC": "low_shelf", "HSC": "high_shelf"}
    bands: List[FilterBand] = []
    for line in text.splitlines():
        m = _PARAM_RE.match(line.strip())
        if not m:
            continue
        bands.append(
            FilterBand(
                type=type_map[m.group(1).upper()],
                frequency=float(m.group(2)),
                gain=float(m.group(3)),
                q=float(m.group(4)),
            ).clamped()
        )
    return bands


def export_parametric_eq(bands: Sequence[FilterBand]) -> str:
    code_map = {"peaking": "PK", "low_shelf": "LSC", "high_shelf": "HSC"}
    lines = ["Preamp: 0.0 dB"]
    for i, b in enumerate(bands, start=1):
        bc = b.clamped()
        lines.append(
            f"Filter {i}: ON {code_map[bc.type]} Fc {int(round(bc.frequency))} Hz "
            f"Gain {bc.gain:.1f} dB Q {bc.q:.3f}"
        )
    return "\n".join(lines) + "\n"


def _center_midband(freqs: np.ndarray, curve_db: np.ndarray) -> np.ndarray:
    mask = (freqs >= 200.0) & (freqs <= 3000.0)
    offset = float(np.mean(curve_db[mask])) if np.any(mask) else float(np.mean(curve_db))
    return curve_db - offset


def _normalize_800hz(freqs: np.ndarray, curve_db: np.ndarray) -> np.ndarray:
    mask = (freqs >= 400.0) & (freqs <= 1200.0)
    offset = float(np.mean(curve_db[mask])) if np.any(mask) else 0.0
    return curve_db - offset


def _default_autoeq_baseline(freqs: np.ndarray, form_factor: str) -> np.ndarray:
    from biquad import band_response_db
    if form_factor == "over-ear":
        f_b, b_db = fetch_target_csv("Harman over-ear 2018.csv")
        return np.interp(np.log10(freqs), np.log10(f_b), b_db)
    f_b, b_db = fetch_target_csv("AutoEq in-ear.csv")
    base = np.interp(np.log10(freqs), np.log10(f_b), b_db)
    bass_shelf = band_response_db(FilterBand("low_shelf", 105.0, 8.0, 0.7), freqs)
    return base + bass_shelf


def _try_fetch_squig_channel(base_url: str, filename: str, cache_path: Path) -> Optional[Tuple[np.ndarray, np.ndarray]]:
    try:
        url = base_url + urllib.parse.quote(filename, safe="")
        txt = _fetch_text(url, cache_path, force_refresh=False, timeout=10.0)
        return parse_squig_fr_txt(txt)
    except Exception:
        return None


def reconstruct_fr_curve(entry: AutoEqEntry) -> Tuple[np.ndarray, np.ndarray]:
    """Return unified normalized acoustic FR (dB, 0 dB @ 400..1200 Hz) on STANDARD_GRID."""
    if entry.source_kind == "squiglink":
        digest = hashlib.sha1(f"{entry.rel_path}|{entry.squig_base_url}".encode("utf-8")).hexdigest()[:16]
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            fl = pool.submit(_try_fetch_squig_channel, entry.squig_base_url, f"{entry.squig_file_stem} L.txt", CACHE_DIR / "profiles" / f"{digest}_L.txt")
            fr_fut = pool.submit(_try_fetch_squig_channel, entry.squig_base_url, f"{entry.squig_file_stem} R.txt", CACHE_DIR / "profiles" / f"{digest}_R.txt")
            left = fl.result()
            right = fr_fut.result()

        log_grid = np.log10(STANDARD_GRID)
        if left and right:
            li = np.interp(log_grid, np.log10(left[0]), left[1])
            ri = np.interp(log_grid, np.log10(right[0]), right[1])
            merged = 0.5 * (li + ri)
        elif left:
            merged = np.interp(log_grid, np.log10(left[0]), left[1])
        elif right:
            merged = np.interp(log_grid, np.log10(right[0]), right[1])
        else:
            mono = _try_fetch_squig_channel(entry.squig_base_url, f"{entry.squig_file_stem}.txt", CACHE_DIR / "profiles" / f"{digest}_M.txt")
            if not mono:
                raise RuntimeError(f"No se pudo descargar la curva FR de {entry.name} ({entry.source_rig}).")
            merged = np.interp(log_grid, np.log10(mono[0]), mono[1])
        return STANDARD_GRID, _normalize_800hz(STANDARD_GRID, merged)

    freqs, eq_db = fetch_graphic_eq(entry)
    ref = _default_autoeq_baseline(freqs, entry.form_factor)
    fr = ref - eq_db
    interp = np.interp(np.log10(STANDARD_GRID), np.log10(freqs), fr)
    return STANDARD_GRID, _normalize_800hz(STANDARD_GRID, interp)


def reconstruct_target_csv_fr(csv_name: Optional[str], form_factor: str = "in-ear") -> Tuple[np.ndarray, np.ndarray]:
    if not csv_name:
        ref = _default_autoeq_baseline(STANDARD_GRID, form_factor)
        return STANDARD_GRID, _normalize_800hz(STANDARD_GRID, ref)
    freqs, ref = fetch_target_csv(csv_name)
    interp = np.interp(np.log10(STANDARD_GRID), np.log10(freqs), ref)
    return STANDARD_GRID, _normalize_800hz(STANDARD_GRID, interp)


def compute_match_headphone_peq(
    source_entry: AutoEqEntry,
    target_phone: AutoEqEntry,
    max_freq_fit: float = 10000.0,
    max_bands: int = 10,
    max_gain_db: float = 12.0,
    supports_shelves: bool = True,
) -> Tuple[np.ndarray, np.ndarray, List[FilterBand], float, Optional[str]]:
    """Compute PEQ making ``source_entry`` match ``target_phone`` across AutoEq & Squiglink."""
    f_src, fr_src = reconstruct_fr_curve(source_entry)
    f_tgt, fr_tgt = reconstruct_fr_curve(target_phone)
    fr_tgt_i = np.interp(np.log10(f_src), np.log10(f_tgt), fr_tgt)

    delta_db = _center_midband(f_src, fr_tgt_i - fr_src)
    bands, rmse = fit_peq_bands(
        f_src,
        delta_db,
        max_bands=max_bands,
        max_freq_fit=max_freq_fit,
        max_gain_db=max_gain_db,
        supports_shelves=supports_shelves,
    )

    warning: Optional[str] = None
    if source_entry.form_factor != target_phone.form_factor:
        warning = (
            f"Aviso: comparando {source_entry.form_factor} vs {target_phone.form_factor}; "
            "el balance tonal relativo se mantiene, pero el oído percibe distinto over-ear vs IEM."
        )
    elif source_entry.rig_tag != target_phone.rig_tag:
        warning = (
            f"Aviso: distinto equipo de medición ({source_entry.rig_tag} vs {target_phone.rig_tag}); "
            "pueden variar los agudos >6 kHz."
        )
    return f_src, delta_db, bands, rmse, warning


def compute_target_curve_peq(
    source_entry: AutoEqEntry,
    target_csv_name: Optional[str],
    max_freq_fit: float = 10000.0,
    max_bands: int = 10,
    max_gain_db: float = 12.0,
    supports_shelves: bool = True,
) -> Tuple[np.ndarray, np.ndarray, List[FilterBand], float]:
    """Compute PEQ for ``source_entry`` toward ``target_csv_name`` (None = AutoEq default)."""
    f_src, fr_src = reconstruct_fr_curve(source_entry)
    f_tgt, fr_tgt = reconstruct_target_csv_fr(target_csv_name, source_entry.form_factor)
    fr_tgt_i = np.interp(np.log10(f_src), np.log10(f_tgt), fr_tgt)
    desired_eq = _center_midband(f_src, fr_tgt_i - fr_src)

    if (
        source_entry.source_kind == "autoeq"
        and target_csv_name is None
        and max_bands >= 5
        and supports_shelves
        and max_gain_db >= 12.0
    ):
        try:
            pre_bands = fetch_parametric_eq(source_entry)[:max_bands]
            if len(pre_bands) >= min(5, max_bands) and all(abs(b.gain) <= max_gain_db for b in pre_bands):
                resp = combined_response_db(pre_bands, f_src)
                resp_mid = _center_midband(f_src, resp)
                w = _make_weights(f_src, max_freq_fit)
                pre_rmse = _weighted_rmse(desired_eq - resp_mid, w)
                if pre_rmse < 0.75:
                    pad_freqs = [31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0]
                    while len(pre_bands) < max_bands:
                        pre_bands.append(FilterBand("peaking", pad_freqs[len(pre_bands) % len(pad_freqs)], 0.0, 1.0))
                    return f_src, desired_eq, pre_bands[:max_bands], pre_rmse
        except Exception:
            pass

    bands, rmse = fit_peq_bands(
        f_src,
        desired_eq,
        max_bands=max_bands,
        max_freq_fit=max_freq_fit,
        max_gain_db=max_gain_db,
        supports_shelves=supports_shelves,
    )
    return f_src, desired_eq, bands, rmse
