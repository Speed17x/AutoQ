#!/usr/bin/env python3
"""AutoQ — Nothing OS Style Parametric EQ Widget & Full App for Hyprland / Wayland.

Supports:
  - Direct USB-C HID DSP (Moondrop 5B / 8B) + Universal 10-band PipeWire PEQ
    for ANY Bluetooth headphone/TWS, generic USB DAC, or 3.5mm wired output.
  - 8 open FR databases (AutoEq GitHub + 7 Squiglink repositories, 14,500+ curves).
  - Compact Nothing OS floating/pinned widget (`--widget`, default) AND full
    3-tab industrial minimalist app (`--full`), switching seamlessly in 1 click.
  - Single-instance Unix socket IPC (`--toggle`, `--widget`, `--full`) for
    Hyprland keybinds (`SUPER+E`) and Waybar click actions (`--status-waybar`).
"""

from __future__ import annotations

import argparse
import base64
import json
import math
import os
import socket
import subprocess
import sys
import threading
import unicodedata
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from PySide6.QtCore import (
    QPoint,
    QPointF,
    QRectF,
    QSize,
    Qt,
    QThread,
    QTimer,
    Signal,
)
from PySide6.QtGui import (
    QBrush,
    QColor,
    QFont,
    QLinearGradient,
    QMouseEvent,
    QPainter,
    QPainterPath,
    QPen,
    QWheelEvent,
)
from PySide6.QtWidgets import (
    QApplication,
    QComboBox,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QPlainTextEdit,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QSlider,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

# ── Nothing OS Color System ──────────────────────────────────────────────────
OBSIDIAN = "#0B0C0E"
SURFACE = "#141517"
SURFACE_HIGH = "#1C1E22"
SURFACE_PILL = "#202227"
GRAPH_BG = "#0E0F12"
BORDER = "#2B2E36"
BORDER_BRIGHT = "#3C404B"
NOTHING_RED = "#D71921"
NOTHING_RED_HOVER = "#EA262F"
PURE_WHITE = "#F5F5F5"
MUTED_GRAY = "#8E929B"
DIM_GRAY = "#585C66"
DOT_GRID = "#262930"
STOCK_LINE = "#6E7480"
TARGET_LINE = "#D8DCE4"

BASE_DIR = Path(__file__).resolve().parent
BACKEND_SCRIPT = BASE_DIR / "backend" / "moondrop_backend.py"
SOCK_PATH = Path(os.environ.get("XDG_RUNTIME_DIR", "/tmp")) / "autoq-hyprland.sock"

# ── 5x7 Nothing OS Dot-Matrix Font Glyphs ────────────────────────────────────
DOT_GLYPHS: Dict[str, Tuple[str, ...]] = {
    "A": (".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"),
    "B": ("####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."),
    "C": (".####", "#....", "#....", "#....", "#....", "#....", ".####"),
    "D": ("####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."),
    "E": ("#####", "#....", "#....", "####.", "#....", "#....", "#####"),
    "F": ("#####", "#....", "#....", "####.", "#....", "#....", "#...."),
    "G": (".####", "#....", "#....", "#..##", "#...#", "#...#", ".####"),
    "H": ("#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"),
    "I": (".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."),
    "J": ("..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."),
    "K": ("#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"),
    "L": ("#....", "#....", "#....", "#....", "#....", "#....", "#####"),
    "M": ("#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#"),
    "N": ("#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"),
    "O": (".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
    "P": ("####.", "#...#", "#...#", "####.", "#....", "#....", "#...."),
    "Q": (".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"),
    "R": ("####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"),
    "S": (".####", "#....", "#....", ".###.", "....#", "....#", "####."),
    "T": ("#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."),
    "U": ("#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
    "V": ("#...#", "#...#", "#...#", "#...#", ".#.#.", ".#.#.", "..#.."),
    "W": ("#...#", "#...#", "#...#", "#...#", "#.#.#", "##.##", "#...#"),
    "X": ("#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"),
    "Y": ("#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."),
    "Z": ("#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"),
    "0": (".###.", "#..##", "#.#.#", "#.#.#", "##..#", "#...#", ".###."),
    "1": ("..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."),
    "2": (".###.", "#...#", "....#", "..##.", ".#...", "#....", "#####"),
    "3": ("####.", "....#", "....#", ".###.", "....#", "....#", "####."),
    "4": ("...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."),
    "5": ("#####", "#....", "####.", "....#", "....#", "#...#", ".###."),
    "6": (".###.", "#....", "####.", "#...#", "#...#", "#...#", ".###."),
    "7": ("#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."),
    "8": (".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."),
    "9": (".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."),
    "-": (".....", ".....", ".....", "#####", ".....", ".....", "....."),
    ".": (".....", ".....", ".....", ".....", ".....", ".##..", ".##.."),
    "/": ("....#", "...#.", "...#.", "..#..", ".#...", ".#...", "#...."),
    "·": (".....", ".....", ".....", "..#..", ".....", ".....", "....."),
    " ": (".....", ".....", ".....", ".....", ".....", ".....", "....."),
}

BRAND_CHIPS = [
    ("Todos", ""),
    ("7Hz", "7hz"),
    ("Moondrop", "moondrop"),
    ("Sony", "sony"),
    ("Huawei", "huawei"),
    ("Nothing", "nothing"),
    ("Apple", "apple"),
    ("Samsung", "samsung"),
    ("Truthear", "truthear"),
    ("KZ", "kz"),
    ("Sennheiser", "sennheiser"),
]


def to_b64(text: str) -> str:
    return base64.b64encode(text.encode("utf-8")).decode("ascii")


def biquad_db_at(band: Dict[str, Any], freq: float) -> float:
    g = float(band.get("gain", 0.0))
    if abs(g) < 0.01:
        return 0.0
    fs = 48000.0
    f0 = max(10.0, min(22000.0, float(band.get("frequency", 1000.0))))
    q = max(0.1, min(10.0, float(band.get("q", 1.0))))
    A = 10.0 ** (g / 40.0)
    w0 = 2.0 * math.pi * f0 / fs
    cw = math.cos(w0)
    sw = math.sin(w0)
    alpha = sw / (2.0 * q)
    ftype = band.get("type", "peaking")
    if ftype == "low_shelf":
        s = 2.0 * math.sqrt(A) * alpha
        b0 = A * ((A + 1) - (A - 1) * cw + s)
        b1 = 2 * A * ((A - 1) - (A + 1) * cw)
        b2 = A * ((A + 1) - (A - 1) * cw - s)
        a0 = (A + 1) + (A - 1) * cw + s
        a1 = -2 * ((A - 1) + (A + 1) * cw)
        a2 = (A + 1) + (A - 1) * cw - s
    elif ftype == "high_shelf":
        s = 2.0 * math.sqrt(A) * alpha
        b0 = A * ((A + 1) + (A - 1) * cw + s)
        b1 = -2 * A * ((A - 1) + (A + 1) * cw)
        b2 = A * ((A + 1) + (A - 1) * cw - s)
        a0 = (A + 1) - (A - 1) * cw + s
        a1 = 2 * ((A - 1) - (A + 1) * cw)
        a2 = (A + 1) - (A - 1) * cw - s
    else:
        b0 = 1 + alpha * A
        b1 = -2 * cw
        b2 = 1 - alpha * A
        a0 = 1 + alpha / A
        a1 = -2 * cw
        a2 = 1 - alpha / A

    b0n, b1n, b2n = b0 / a0, b1 / a0, b2 / a0
    a1n, a2n = a1 / a0, a2 / a0
    w = 2.0 * math.pi * freq / fs
    phi = math.sin(w / 2.0) ** 2
    num = (
        (b0n + b1n + b2n) ** 2
        - 4 * (b0n * b1n + 4 * b0n * b2n + b1n * b2n) * phi
        + 16 * b0n * b2n * phi * phi
    )
    den = (
        (1 + a1n + a2n) ** 2
        - 4 * (a1n + 4 * a2n + a1n * a2n) * phi
        + 16 * a2n * phi * phi
    )
    return 10.0 * math.log10(max(num, 1e-18) / max(den, 1e-18))


def interp_curve(pts: List[List[float]], freq: float) -> float:
    if not pts:
        return 0.0
    if freq <= pts[0][0]:
        return float(pts[0][1])
    if freq >= pts[-1][0]:
        return float(pts[-1][1])
    lf = math.log(freq)
    lo, hi = 0, len(pts) - 1
    while lo + 1 < hi:
        mid = (lo + hi) // 2
        if pts[mid][0] <= freq:
            lo = mid
        else:
            hi = mid
    lf0 = math.log(max(1.0, pts[lo][0]))
    lf1 = math.log(max(1.0, pts[hi][0]))
    t = (lf - lf0) / max(1e-9, lf1 - lf0)
    return float(pts[lo][1] + t * (pts[hi][1] - pts[lo][1]))


def fmt_freq(f: float) -> str:
    if f >= 1000.0:
        return f"{f / 1000.0:.1f}k" if f < 10000.0 else f"{int(round(f / 1000.0))}k"
    return f"{int(round(f))}"


# ── Background Subprocess Runner (Non-blocking UI) ───────────────────────────
class BackendWorker(QThread):
    finished_json = Signal(str, dict)

    def __init__(self, tag: str, args: List[str], parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self.tag = tag
        self.args = args

    def run(self) -> None:
        try:
            proc = subprocess.run(
                [sys.executable, str(BACKEND_SCRIPT), *self.args],
                capture_output=True,
                text=True,
                timeout=35,
            )
            raw = (proc.stdout or "").strip()
            if not raw:
                err = (proc.stderr or "").strip().splitlines()
                self.finished_json.emit(
                    self.tag,
                    {"ok": False, "error": err[0] if err else "Sin respuesta del backend"},
                )
                return
            data = json.loads(raw)
            self.finished_json.emit(self.tag, data)
        except Exception as exc:
            self.finished_json.emit(self.tag, {"ok": False, "error": str(exc)})


# ── Nothing OS 5×7 Dot-Matrix Header Label ───────────────────────────────────
class DotMatrixLabel(QWidget):
    def __init__(
        self,
        text: str = "AUTOQ",
        dot_step: float = 2.6,
        color: str = PURE_WHITE,
        max_chars: int = 16,
        parent: Optional[QWidget] = None,
    ) -> None:
        super().__init__(parent)
        self._text = text.upper()
        self._step = dot_step
        self._color = QColor(color)
        self._max_chars = max_chars
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self.setFixedHeight(int(round(7 * self._step + 4)))

    @staticmethod
    def _clean_text(text: str) -> str:
        norm = unicodedata.normalize("NFKD", (text or "AUTOQ").strip())
        ascii_like = "".join(ch for ch in norm if not unicodedata.combining(ch))
        return ascii_like.upper() or "AUTOQ"

    def setText(self, text: str) -> None:
        clean = self._clean_text(text)
        if clean != self._text:
            self._text = clean
            self.update()

    def sizeHint(self) -> QSize:
        n = max(4, min(len(self._text), self._max_chars))
        w = int(n * 6.2 * self._step + 4)
        return QSize(w, int(round(7 * self._step + 4)))

    def paintEvent(self, _event) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing, True)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QBrush(self._color))

        char_adv = 6.2 * self._step
        max_fit = max(3, int((self.width() - 4) // char_adv))
        display = self._text[: min(self._max_chars, max_fit)]
        r = self._step * 0.37
        y0 = (self.height() - 6 * self._step) / 2.0

        for idx, ch in enumerate(display):
            glyph = DOT_GLYPHS.get(ch, DOT_GLYPHS[" "])
            bx = 2.0 + idx * char_adv
            for row_i, row in enumerate(glyph):
                for col_i, cell in enumerate(row):
                    if cell == "#":
                        p.drawEllipse(
                            QPointF(bx + col_i * self._step, y0 + row_i * self._step),
                            r,
                            r,
                        )
        p.end()


# ── Nothing OS Dot-Grid Interactive EQ/FR Curve Canvas ───────────────────────
class NothingEqCanvas(QWidget):
    band_selected = Signal(int)
    band_dragged = Signal(int, float, float)  # idx, freq, gain
    band_q_changed = Signal(int, float)
    canvas_clicked = Signal()

    def __init__(self, interactive: bool = True, parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self.interactive = interactive
        self.bands: List[Dict[str, Any]] = []
        self.selected_band: int = 0
        self.dsp_enabled: bool = True
        self.overlay_fr: bool = True
        self.stock_fr: List[List[float]] = []
        self.target_fr: List[List[float]] = []
        self.target_delta: List[List[float]] = []
        self.target_label: str = ""
        self.rmse_text: str = ""
        self.max_gain_db: float = 12.0
        self._drag_idx: int = -1
        self.setMinimumHeight(94 if not interactive else 178)
        self.setCursor(Qt.CursorShape.PointingHandCursor if not interactive else Qt.CursorShape.CrossCursor)

    def set_state(
        self,
        bands: List[Dict[str, Any]],
        selected_band: int,
        dsp_enabled: bool,
        overlay_fr: bool,
        stock_fr: List[List[float]],
        target_fr: List[List[float]],
        target_delta: List[List[float]],
        target_label: str,
        rmse_text: str,
        max_gain_db: float = 12.0,
    ) -> None:
        self.bands = bands
        self.selected_band = selected_band
        self.dsp_enabled = dsp_enabled
        self.overlay_fr = overlay_fr
        self.stock_fr = stock_fr
        self.target_fr = target_fr
        self.target_delta = target_delta
        self.target_label = target_label
        self.rmse_text = rmse_text
        self.max_gain_db = max_gain_db
        self.update()

    def _has_stock(self) -> bool:
        return self.overlay_fr and len(self.stock_fr) > 1

    def _db_bounds(self) -> Tuple[float, float]:
        return (-12.0, 17.0) if self._has_stock() else (-15.0, 15.0)

    def _plot_rect(self) -> QRectF:
        pad_l = 24.0 if self.interactive else 10.0
        pad_r = 10.0
        pad_t = 18.0 if self.interactive else 12.0
        pad_b = 18.0 if self.interactive else 10.0
        return QRectF(pad_l, pad_t, max(20.0, self.width() - pad_l - pad_r), max(20.0, self.height() - pad_t - pad_b))

    def _fx(self, freq: float, rect: QRectF) -> float:
        lo, hi = math.log10(20.0), math.log10(20000.0)
        lf = math.log10(max(20.0, min(20000.0, freq)))
        return rect.left() + ((lf - lo) / (hi - lo)) * rect.width()

    def _xf(self, x: float, rect: QRectF) -> float:
        lo, hi = math.log10(20.0), math.log10(20000.0)
        t = max(0.0, min(1.0, (x - rect.left()) / max(1.0, rect.width())))
        return 10.0 ** (lo + t * (hi - lo))

    def _gy(self, db: float, rect: QRectF) -> float:
        lo, hi = self._db_bounds()
        return rect.top() + (1.0 - (db - lo) / (hi - lo)) * rect.height()

    def _yg(self, y: float, rect: QRectF) -> float:
        lo, hi = self._db_bounds()
        return lo + (1.0 - (y - rect.top()) / max(1.0, rect.height())) * (hi - lo)

    def _total_eq(self, freq: float) -> float:
        if not self.dsp_enabled:
            return 0.0
        return sum(biquad_db_at(b, freq) for b in self.bands)

    def paintEvent(self, _event) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing, True)

        # Recessed Nothing matte obsidian canvas card
        rad_card = 16.0 if not self.interactive else 18.0
        bg_rect = QRectF(0.5, 0.5, self.width() - 1.0, self.height() - 1.0)
        clip_path = QPainterPath()
        clip_path.addRoundedRect(bg_rect.adjusted(1.0, 1.0, -1.0, -1.0), rad_card - 1.0, rad_card - 1.0)
        p.setPen(QPen(QColor(BORDER), 1.0))
        p.setBrush(QBrush(QColor(GRAPH_BG)))
        p.drawRoundedRect(bg_rect, rad_card, rad_card)
        p.save()
        p.setClipPath(clip_path)

        rect = self._plot_rect()
        has_stock = self._has_stock()

        # Signature Nothing OS Dot-Field Grid
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QBrush(QColor(DOT_GRID)))
        cols = 20 if self.interactive else 16
        rows = 7 if self.interactive else 5
        for r in range(rows):
            yy = rect.top() + (r / max(1, rows - 1)) * rect.height()
            for c in range(cols):
                xx = rect.left() + (c / max(1, cols - 1)) * rect.width()
                p.drawEllipse(QPointF(xx, yy), 1.15, 1.15)

        # 0 dB hairline reference
        y0 = self._gy(0.0, rect)
        p.setPen(QPen(QColor("#363A44"), 1.0, Qt.PenStyle.DashLine))
        p.drawLine(QPointF(rect.left(), y0), QPointF(rect.right(), y0))

        # Axis labels (interactive mode)
        if self.interactive:
            font = QFont("JetBrains Mono, FantasqueSansM Nerd Font Mono, monospace")
            font.setPixelSize(9)
            p.setFont(font)
            p.setPen(QColor(DIM_GRAY))
            for f, lbl in [(20, "20"), (100, "100"), (500, "500"), (1000, "1k"), (5000, "5k"), (20000, "20k")]:
                xx = self._fx(f, rect)
                p.drawText(QRectF(xx - 16, rect.bottom() + 2, 32, 13), Qt.AlignmentFlag.AlignCenter, lbl)
            for db_val in (-10, 0, 10) if has_stock else (-12, 0, 12):
                yy = self._gy(db_val, rect)
                txt = f"{db_val:+d}" if db_val != 0 else "0"
                p.drawText(QRectF(2, yy - 6, rect.left() - 5, 12), Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter, txt)

        # 1) Stock FR curve (dashed muted gray)
        if has_stock:
            path_s = QPainterPath()
            for i, pt in enumerate(self.stock_fr):
                x, y = self._fx(pt[0], rect), self._gy(pt[1], rect)
                if i == 0:
                    path_s.moveTo(x, y)
                else:
                    path_s.lineTo(x, y)
            pen_s = QPen(QColor(STOCK_LINE), 1.35, Qt.PenStyle.DashLine)
            pen_s.setDashPattern([4.0, 3.0])
            p.setPen(pen_s)
            p.setBrush(Qt.BrushStyle.NoBrush)
            p.drawPath(path_s)

        # 2) Target curve (dotted crisp white)
        overlay_tgt = self.target_fr if has_stock else self.target_delta
        if len(overlay_tgt) > 1:
            path_t = QPainterPath()
            for i, pt in enumerate(overlay_tgt):
                x, y = self._fx(pt[0], rect), self._gy(pt[1], rect)
                if i == 0:
                    path_t.moveTo(x, y)
                else:
                    path_t.lineTo(x, y)
            pen_t = QPen(QColor(TARGET_LINE), 1.4, Qt.PenStyle.DotLine)
            p.setPen(pen_t)
            p.setBrush(Qt.BrushStyle.NoBrush)
            p.drawPath(path_t)

        # 3) Live EQ / FR curve (Nothing Red with subtle red glow + gradient fill)
        n_pts = 96
        lo_log, hi_log = math.log10(20.0), math.log10(20000.0)
        pts: List[QPointF] = []
        for k in range(n_pts):
            fr = 10.0 ** (lo_log + (k / (n_pts - 1)) * (hi_log - lo_log))
            val = self._total_eq(fr)
            if has_stock:
                val += interp_curve(self.stock_fr, fr)
            pts.append(QPointF(self._fx(fr, rect), self._gy(val, rect)))

        accent = QColor(NOTHING_RED) if self.dsp_enabled else QColor("#676C77")

        # Soft fill
        fill_path = QPainterPath()
        base_y = rect.bottom() if has_stock else y0
        fill_path.moveTo(pts[0].x(), base_y)
        for pt in pts:
            fill_path.lineTo(pt)
        fill_path.lineTo(pts[-1].x(), base_y)
        fill_path.closeSubpath()

        grad = QLinearGradient(0, rect.top(), 0, rect.bottom())
        c_top = QColor(accent)
        c_top.setAlpha(58 if self.dsp_enabled else 26)
        c_bot = QColor(accent)
        c_bot.setAlpha(4)
        grad.setColorAt(0.0, c_top)
        grad.setColorAt(1.0, c_bot)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QBrush(grad))
        p.drawPath(fill_path)

        # Glow + crisp stroke
        stroke_path = QPainterPath()
        stroke_path.moveTo(pts[0])
        for pt in pts[1:]:
            stroke_path.lineTo(pt)

        glow_c = QColor(accent)
        glow_c.setAlpha(72 if self.dsp_enabled else 30)
        p.setBrush(Qt.BrushStyle.NoBrush)
        p.setPen(QPen(glow_c, 5.2, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap, Qt.PenJoinStyle.RoundJoin))
        p.drawPath(stroke_path)

        p.setPen(QPen(accent, 2.2, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap, Qt.PenJoinStyle.RoundJoin))
        p.drawPath(stroke_path)

        # Band nodes
        for idx, b in enumerate(self.bands):
            bx = self._fx(float(b.get("frequency", 1000.0)), rect)
            gval = float(b.get("gain", 0.0)) if self.dsp_enabled else 0.0
            if has_stock:
                gval += interp_curve(self.stock_fr, float(b.get("frequency", 1000.0)))
            by = self._gy(gval, rect)
            is_sel = self.interactive and (idx == self.selected_band)
            rad = 6.2 if is_sel else (3.6 if self.interactive else 2.6)
            p.setBrush(QBrush(accent if (is_sel or abs(gval) > 0.05) else QColor(PURE_WHITE)))
            p.setPen(QPen(QColor(PURE_WHITE), 1.6) if is_sel else Qt.PenStyle.NoPen)
            p.drawEllipse(QPointF(bx, by), rad, rad)

        # Top HUD legend
        lfont = QFont("JetBrains Mono, FantasqueSansM Nerd Font Mono, monospace")
        lfont.setPixelSize(9)
        lfont.setBold(True)
        p.setFont(lfont)

        p.setPen(accent)
        p.drawText(QPointF(rect.left() + 3, 13.0 if self.interactive else 11.0), "● LIVE" if self.dsp_enabled else "○ BYPASS")
        if has_stock:
            p.setPen(QColor(MUTED_GRAY))
            p.drawText(QPointF(rect.left() + 56, 13.0 if self.interactive else 11.0), "╌ STOCK")
        if len(overlay_tgt) > 1 and self.interactive:
            p.setPen(QColor(PURE_WHITE))
            lbl = (self.target_label[:14].upper() if self.target_label else "TARGET")
            p.drawText(QPointF(rect.left() + 114, 13.0), f"┈ {lbl}")
        if self.rmse_text:
            p.setPen(QColor(MUTED_GRAY))
            p.drawText(QRectF(rect.left(), 2.0, rect.width() - 3, 13.0), Qt.AlignmentFlag.AlignRight, self.rmse_text)

        p.restore()
        p.end()

    def mousePressEvent(self, event: QMouseEvent) -> None:
        if not self.interactive:
            self.canvas_clicked.emit()
            return
        rect = self._plot_rect()
        has_stock = self._has_stock()
        pos = event.position()
        best = -1
        best_d2 = 18.0 * 18.0
        for i, b in enumerate(self.bands):
            bx = self._fx(float(b.get("frequency", 1000.0)), rect)
            gval = float(b.get("gain", 0.0)) if self.dsp_enabled else 0.0
            if has_stock:
                gval += interp_curve(self.stock_fr, float(b.get("frequency", 1000.0)))
            by = self._gy(gval, rect)
            d2 = (pos.x() - bx) ** 2 + (pos.y() - by) ** 2
            if d2 < best_d2:
                best_d2 = d2
                best = i
        if best >= 0:
            self._drag_idx = best
            self.selected_band = best
            self.band_selected.emit(best)
            self.update()

    def mouseMoveEvent(self, event: QMouseEvent) -> None:
        if not self.interactive or self._drag_idx < 0:
            return
        rect = self._plot_rect()
        pos = event.position()
        new_freq = round(self._xf(pos.x(), rect))
        y_db = self._yg(pos.y(), rect)
        has_stock = self._has_stock()
        gain = y_db - (interp_curve(self.stock_fr, new_freq) if has_stock else 0.0)
        gain = round(max(-self.max_gain_db, min(self.max_gain_db, gain)), 1)
        self.band_dragged.emit(self._drag_idx, float(new_freq), float(gain))

    def mouseReleaseEvent(self, _event: QMouseEvent) -> None:
        self._drag_idx = -1

    def mouseDoubleClickEvent(self, _event: QMouseEvent) -> None:
        if self.interactive and 0 <= self.selected_band < len(self.bands):
            b = self.bands[self.selected_band]
            self.band_dragged.emit(self.selected_band, float(b["frequency"]), 0.0)

    def wheelEvent(self, event: QWheelEvent) -> None:
        if not self.interactive or not (0 <= self.selected_band < len(self.bands)):
            return
        delta = 0.1 if event.angleDelta().y() > 0 else -0.1
        cur_q = float(self.bands[self.selected_band].get("q", 1.0))
        new_q = round(max(0.1, min(10.0, cur_q + delta)), 2)
        self.band_q_changed.emit(self.selected_band, new_q)


# ── Main Nothing OS Window (Compact Widget + Full App) ───────────────────────
class AutoQNothingWindow(QWidget):
    ipc_command = Signal(str)

    def __init__(self, start_full: bool = False) -> None:
        super().__init__()
        self.setObjectName("autoq-widget")
        self.setWindowTitle("AutoQ")
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)

        self.full_mode: bool = start_full
        self.active_device: Optional[Dict[str, Any]] = None
        self.all_devices: List[Dict[str, Any]] = []
        self.bands: List[Dict[str, Any]] = self._flat_bands(10)
        self.selected_band: int = 0
        self.dsp_enabled: bool = True
        self.live_preview: bool = True
        self.overlay_fr: bool = True
        self.pipewire_active: bool = False
        self.dirty_bands: bool = False
        self.busy: bool = False

        self.total_db_count: int = 0
        self.active_sources: int = 0
        self.total_sources: int = 8
        self.selected_source: Optional[Dict[str, Any]] = None
        self.targets: List[Dict[str, str]] = []
        self.presets: Dict[str, List[Dict[str, Any]]] = {}
        self.preset_names: List[str] = []
        self.active_preset_idx: int = 0

        self.stock_fr: List[List[float]] = []
        self.target_fr: List[List[float]] = []
        self.target_delta: List[List[float]] = []
        self.target_label: str = ""
        self.rmse_text: str = ""
        self.rig_warning: str = ""
        self.autoq_mode: str = "target"  # "target" | "match"
        self.max_freq_fit: int = 10000

        self._workers: List[BackendWorker] = []
        self._drag_offset: Optional[QPoint] = None
        self._updating_controls: bool = False
        self._source_results: List[Dict[str, Any]] = []
        self._match_results: List[Dict[str, Any]] = []

        self._preview_timer = QTimer(self)
        self._preview_timer.setSingleShot(True)
        self._preview_timer.setInterval(180)
        self._preview_timer.timeout.connect(lambda: self.apply_bands(False))

        self._src_search_timer = QTimer(self)
        self._src_search_timer.setSingleShot(True)
        self._src_search_timer.setInterval(160)
        self._src_search_timer.timeout.connect(self._do_source_search)

        self._match_search_timer = QTimer(self)
        self._match_search_timer.setSingleShot(True)
        self._match_search_timer.setInterval(160)
        self._match_search_timer.timeout.connect(self._do_match_search)

        self._build_ui()
        self._apply_stylesheet()
        self.set_mode(start_full)
        self.ipc_command.connect(self._handle_ipc)

    @staticmethod
    def _flat_bands(n: int = 10) -> List[Dict[str, Any]]:
        freqs = (
            [60, 250, 1000, 3500, 8000]
            if n <= 5
            else [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
        )
        return [{"type": "peaking", "frequency": freqs[i % len(freqs)], "gain": 0.0, "q": 1.0} for i in range(n)]

    def _fit_bands_to_device(self, raw: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        max_b = int(self.active_device["max_bands"]) if self.active_device else 10
        pad = self._flat_bands(max_b)
        out = [dict(b) for b in raw[:max_b]]
        while len(out) < max_b:
            out.append(pad[len(out)])
        return out

    # ── UI Construction ──────────────────────────────────────────────────────
    def _build_ui(self) -> None:
        outer = QVBoxLayout(self)
        outer.setContentsMargins(6, 6, 6, 6)

        self.shell = QFrame()
        self.shell.setObjectName("nothingShell")
        outer.addWidget(self.shell)

        shell_lay = QVBoxLayout(self.shell)
        shell_lay.setContentsMargins(0, 0, 0, 0)
        shell_lay.setSpacing(0)

        self.mode_stack = QStackedWidget()
        shell_lay.addWidget(self.mode_stack)

        self.compact_page = self._build_compact_widget()
        self.full_page = self._build_full_app()
        self.mode_stack.addWidget(self.compact_page)
        self.mode_stack.addWidget(self.full_page)

    def _build_compact_widget(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(16, 14, 16, 14)
        lay.setSpacing(10)

        # Header: Red status dot + Dot-Matrix Title + Transport Badge + Red ON/OFF
        hdr = QHBoxLayout()
        hdr.setSpacing(8)

        self.w_dot = QLabel()
        self.w_dot.setFixedSize(9, 9)
        self.w_dot.setObjectName("redDot")
        hdr.addWidget(self.w_dot)

        self.w_title = DotMatrixLabel("AUTOQ", dot_step=2.5, max_chars=13)
        hdr.addWidget(self.w_title, 1)

        self.w_badge = QLabel("AUX·10B")
        self.w_badge.setObjectName("badgePill")
        hdr.addWidget(self.w_badge)

        self.w_power_btn = QPushButton("ON")
        self.w_power_btn.setObjectName("redPowerBtn")
        self.w_power_btn.setFixedSize(42, 26)
        self.w_power_btn.clicked.connect(self.toggle_dsp)
        hdr.addWidget(self.w_power_btn)

        lay.addLayout(hdr)

        # Center: Dot-Grid Curve Canvas (clicking expands to Full App)
        self.w_canvas = NothingEqCanvas(interactive=False)
        self.w_canvas.setFixedHeight(104)
        self.w_canvas.canvas_clicked.connect(lambda: self.set_mode(True))
        lay.addWidget(self.w_canvas)

        # Bottom Hardware Pill Strip: ‹  [Preset Name]  ›   [EQ]
        bot = QHBoxLayout()
        bot.setSpacing(6)

        self.w_prev_btn = QPushButton("‹")
        self.w_prev_btn.setObjectName("circlePillBtn")
        self.w_prev_btn.setFixedSize(32, 30)
        self.w_prev_btn.clicked.connect(lambda: self.cycle_preset(-1))
        bot.addWidget(self.w_prev_btn)

        self.w_preset_pill = QPushButton("Plano (0 dB)")
        self.w_preset_pill.setObjectName("presetPillBtn")
        self.w_preset_pill.setFixedHeight(30)
        self.w_preset_pill.clicked.connect(lambda: self.cycle_preset(1))
        bot.addWidget(self.w_preset_pill, 1)

        self.w_next_btn = QPushButton("›")
        self.w_next_btn.setObjectName("circlePillBtn")
        self.w_next_btn.setFixedSize(32, 30)
        self.w_next_btn.clicked.connect(lambda: self.cycle_preset(1))
        bot.addWidget(self.w_next_btn)

        self.w_expand_btn = QPushButton("EQ")
        self.w_expand_btn.setObjectName("expandEqBtn")
        self.w_expand_btn.setFixedSize(46, 30)
        self.w_expand_btn.clicked.connect(lambda: self.set_mode(True))
        bot.addWidget(self.w_expand_btn)

        lay.addLayout(bot)
        return w

    def _build_full_app(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(16, 14, 16, 12)
        lay.setSpacing(10)

        # ── Top Nothing Header ───────────────────────────────────────────────
        hdr = QHBoxLayout()
        hdr.setSpacing(8)

        self.f_dot = QLabel()
        self.f_dot.setFixedSize(9, 9)
        self.f_dot.setObjectName("redDot")
        hdr.addWidget(self.f_dot)

        self.f_title = DotMatrixLabel("AUTOQ", dot_step=2.7, max_chars=10)
        self.f_title.setFixedWidth(116)
        hdr.addWidget(self.f_title)

        self.f_dev_combo = QComboBox()
        self.f_dev_combo.setObjectName("nothingCombo")
        self.f_dev_combo.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self.f_dev_combo.activated.connect(self._on_device_changed)
        hdr.addWidget(self.f_dev_combo, 1)

        self.f_power_btn = QPushButton("ON")
        self.f_power_btn.setObjectName("redPowerBtn")
        self.f_power_btn.setFixedSize(42, 26)
        self.f_power_btn.clicked.connect(self.toggle_dsp)
        hdr.addWidget(self.f_power_btn)

        self.f_collapse_btn = QPushButton("⤡")
        self.f_collapse_btn.setObjectName("circlePillBtn")
        self.f_collapse_btn.setToolTip("Modo widget compacto Nothing OS")
        self.f_collapse_btn.setFixedSize(28, 26)
        self.f_collapse_btn.clicked.connect(lambda: self.set_mode(False))
        hdr.addWidget(self.f_collapse_btn)

        self.f_close_btn = QPushButton("×")
        self.f_close_btn.setObjectName("circlePillBtn")
        self.f_close_btn.setToolTip("Ocultar ventana (SUPER+E para volver)")
        self.f_close_btn.setFixedSize(28, 26)
        self.f_close_btn.clicked.connect(self.hide)
        hdr.addWidget(self.f_close_btn)

        lay.addLayout(hdr)

        # ── 3-Tab Nothing Segmented Pill Switcher ────────────────────────────
        tab_bar = QFrame()
        tab_bar.setObjectName("tabPillBar")
        tlay = QHBoxLayout(tab_bar)
        tlay.setContentsMargins(4, 4, 4, 4)
        tlay.setSpacing(4)

        self.tab_buttons: List[QPushButton] = []
        for idx, label in enumerate(("EQ", "AUTOQ", "AVANZADO")):
            btn = QPushButton(label)
            btn.setObjectName("tabPillBtn")
            btn.setCheckable(True)
            btn.setChecked(idx == 0)
            btn.clicked.connect(lambda _=False, i=idx: self.switch_tab(i))
            tlay.addWidget(btn, 1)
            self.tab_buttons.append(btn)
        lay.addWidget(tab_bar)

        self.tab_stack = QStackedWidget()
        lay.addWidget(self.tab_stack, 1)

        self.tab_stack.addWidget(self._build_eq_tab())
        self.tab_stack.addWidget(self._build_autoq_tab())
        self.tab_stack.addWidget(self._build_advanced_tab())

        # ── Telemetry Footer ─────────────────────────────────────────────────
        self.status_lbl = QLabel("> AUTOQ READY")
        self.status_lbl.setObjectName("telemetryFooter")
        lay.addWidget(self.status_lbl)
        return w

    def _build_eq_tab(self) -> QWidget:
        page = QWidget()
        lay = QVBoxLayout(page)
        lay.setContentsMargins(0, 2, 0, 0)
        lay.setSpacing(9)

        # Quick action strip
        act = QHBoxLayout()
        act.setSpacing(6)

        self.btn_fr_mode = QPushButton("FR")
        self.btn_fr_mode.setObjectName("pillToggleBtn")
        self.btn_fr_mode.setCheckable(True)
        self.btn_fr_mode.setChecked(True)
        self.btn_fr_mode.clicked.connect(self._toggle_fr_mode)
        act.addWidget(self.btn_fr_mode)

        btn_flat = QPushButton("0 dB")
        btn_flat.setObjectName("pillBtn")
        btn_flat.clicked.connect(self.set_flat)
        act.addWidget(btn_flat)

        btn_read = QPushButton("↻ Leer")
        btn_read.setObjectName("pillBtn")
        btn_read.clicked.connect(lambda: self.refresh_status(False))
        act.addWidget(btn_read)

        self.btn_live = QPushButton("LIVE")
        self.btn_live.setObjectName("pillToggleBtn")
        self.btn_live.setCheckable(True)
        self.btn_live.setChecked(True)
        self.btn_live.clicked.connect(self._toggle_live)
        act.addWidget(self.btn_live)

        act.addStretch(1)

        self.btn_apply = QPushButton("APLICAR")
        self.btn_apply.setObjectName("redActionBtn")
        self.btn_apply.clicked.connect(lambda: self.apply_bands(True))
        act.addWidget(self.btn_apply)

        lay.addLayout(act)

        # Interactive Dot-Grid Curve Canvas
        self.f_canvas = NothingEqCanvas(interactive=True)
        self.f_canvas.setMinimumHeight(184)
        self.f_canvas.band_selected.connect(self.select_band)
        self.f_canvas.band_dragged.connect(self._on_canvas_drag)
        self.f_canvas.band_q_changed.connect(self._on_canvas_q)
        lay.addWidget(self.f_canvas)

        # Band Chips Grid (5 cols: 1 row for 5B, 2 crisp rows for 8B/10B)
        self.band_grid_host = QWidget()
        self.band_grid = QGridLayout(self.band_grid_host)
        self.band_grid.setContentsMargins(0, 0, 0, 0)
        self.band_grid.setHorizontalSpacing(5)
        self.band_grid.setVerticalSpacing(5)
        self.band_chips: List[QPushButton] = []
        lay.addWidget(self.band_grid_host)

        # Selected Band Telemetry Editor Card
        ed_card = QFrame()
        ed_card.setObjectName("subCard")
        ed = QVBoxLayout(ed_card)
        ed.setContentsMargins(12, 10, 12, 10)
        ed.setSpacing(8)

        # Filter type pills + Band label + Zero gain
        top_ed = QHBoxLayout()
        top_ed.setSpacing(6)

        self.ed_band_title = QLabel("BANDA 1")
        self.ed_band_title.setObjectName("sectionLabel")
        top_ed.addWidget(self.ed_band_title)
        top_ed.addStretch(1)

        self.type_btns: Dict[str, QPushButton] = {}
        for code, lbl in (("peaking", "PK"), ("low_shelf", "LS"), ("high_shelf", "HS")):
            b = QPushButton(lbl)
            b.setObjectName("pillToggleBtn")
            b.setCheckable(True)
            b.setFixedWidth(40)
            b.clicked.connect(lambda _=False, c=code: self._set_band_type(c))
            top_ed.addWidget(b)
            self.type_btns[code] = b

        btn_zero_b = QPushButton("0.0")
        btn_zero_b.setObjectName("pillBtn")
        btn_zero_b.setFixedWidth(40)
        btn_zero_b.clicked.connect(lambda: self._update_band_field("gain", 0.0))
        top_ed.addWidget(btn_zero_b)
        ed.addLayout(top_ed)

        # Frequency slider (log scale 20..20000 -> 0..1000)
        self.freq_slider, self.freq_val_lbl = self._add_slider_row(
            ed,
            "FREQ",
            0,
            1000,
            self._on_freq_slider,
            lambda: self._step_freq(-1),
            lambda: self._step_freq(1),
        )
        # Gain slider (-12.0..+12.0 -> -120..120)
        self.gain_slider, self.gain_val_lbl = self._add_slider_row(
            ed,
            "GAIN",
            -120,
            120,
            self._on_gain_slider,
            lambda: self._step_gain(-0.2),
            lambda: self._step_gain(0.2),
        )
        # Q slider (0.10..10.00 -> 10..1000)
        self.q_slider, self.q_val_lbl = self._add_slider_row(
            ed,
            "Q",
            10,
            1000,
            self._on_q_slider,
            lambda: self._step_q(-0.1),
            lambda: self._step_q(0.1),
        )

        lay.addWidget(ed_card)
        lay.addStretch(1)
        return page

    def _add_slider_row(
        self,
        parent_lay: QVBoxLayout,
        label: str,
        min_v: int,
        max_v: int,
        on_change,
        on_minus,
        on_plus,
    ) -> Tuple[QSlider, QLabel]:
        row = QHBoxLayout()
        row.setSpacing(6)

        lbl = QLabel(label)
        lbl.setObjectName("paramLabel")
        lbl.setFixedWidth(36)
        row.addWidget(lbl)

        btn_m = QPushButton("−")
        btn_m.setObjectName("stepBtn")
        btn_m.setFixedSize(22, 22)
        btn_m.clicked.connect(on_minus)
        row.addWidget(btn_m)

        sl = QSlider(Qt.Orientation.Horizontal)
        sl.setObjectName("nothingSlider")
        sl.setRange(min_v, max_v)
        sl.valueChanged.connect(on_change)
        row.addWidget(sl, 1)

        btn_p = QPushButton("+")
        btn_p.setObjectName("stepBtn")
        btn_p.setFixedSize(22, 22)
        btn_p.clicked.connect(on_plus)
        row.addWidget(btn_p)

        val = QLabel("--")
        val.setObjectName("paramValue")
        val.setFixedWidth(64)
        val.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        row.addWidget(val)

        parent_lay.addLayout(row)
        return sl, val

    def _build_autoq_tab(self) -> QWidget:
        page = QWidget()
        lay = QVBoxLayout(page)
        lay.setContentsMargins(0, 2, 0, 0)
        lay.setSpacing(8)

        # Catalog info + Sync bar
        cat_row = QHBoxLayout()
        cat_row.setSpacing(6)
        self.cat_info_lbl = QLabel("CATÁLOGO: CARGANDO...")
        self.cat_info_lbl.setObjectName("sectionLabel")
        cat_row.addWidget(self.cat_info_lbl, 1)

        self.btn_sync_db = QPushButton("↻ Sync 8 Fuentes")
        self.btn_sync_db.setObjectName("pillBtn")
        self.btn_sync_db.clicked.connect(lambda: self.refresh_status(True))
        cat_row.addWidget(self.btn_sync_db)
        lay.addLayout(cat_row)

        # Card 1: Tu Audífono (Universal Source Search)
        src_card = QFrame()
        src_card.setObjectName("subCard")
        sc = QVBoxLayout(src_card)
        sc.setContentsMargins(10, 8, 10, 8)
        sc.setSpacing(6)

        self.src_selected_lbl = QLabel("TU AUDÍFONO (ORIGEN): Sin seleccionar")
        self.src_selected_lbl.setObjectName("highlightLabel")
        self.src_selected_lbl.setWordWrap(True)
        sc.addWidget(self.src_selected_lbl)

        self.src_search_edit = QLineEdit()
        self.src_search_edit.setObjectName("nothingInput")
        self.src_search_edit.setPlaceholderText("Buscar tu audífono (Golden Ages 2, Zero 3, Chu 2, XM5...)")
        self.src_search_edit.textChanged.connect(lambda _: self._src_search_timer.start())
        sc.addWidget(self.src_search_edit)

        # Quick Brand Filter Chips (scrollable strip)
        chip_scroll = QScrollArea()
        chip_scroll.setWidgetResizable(True)
        chip_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        chip_scroll.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        chip_scroll.setFixedHeight(28)
        chip_scroll.setFrameShape(QFrame.Shape.NoFrame)
        chip_scroll.setStyleSheet("background: transparent;")
        chip_host = QWidget()
        chip_host.setStyleSheet("background: transparent;")
        chip_lay = QHBoxLayout(chip_host)
        chip_lay.setContentsMargins(0, 0, 0, 0)
        chip_lay.setSpacing(4)
        for lbl, query in BRAND_CHIPS:
            cb = QPushButton(lbl)
            cb.setObjectName("brandChipBtn")
            cb.clicked.connect(lambda _=False, q=query: self.src_search_edit.setText(q))
            chip_lay.addWidget(cb)
        chip_scroll.setWidget(chip_host)
        sc.addWidget(chip_scroll)

        self.src_list = QListWidget()
        self.src_list.setObjectName("nothingList")
        self.src_list.setFixedHeight(98)
        self.src_list.itemClicked.connect(self._on_source_picked)
        sc.addWidget(self.src_list)
        lay.addWidget(src_card)

        # Mode switch: CURVA TARGET vs IMITAR AUDÍFONO
        mode_row = QHBoxLayout()
        mode_row.setSpacing(6)
        self.btn_mode_target = QPushButton("CURVA TARGET")
        self.btn_mode_target.setObjectName("pillToggleBtn")
        self.btn_mode_target.setCheckable(True)
        self.btn_mode_target.setChecked(True)
        self.btn_mode_target.clicked.connect(lambda: self._set_autoq_mode("target"))
        mode_row.addWidget(self.btn_mode_target, 1)

        self.btn_mode_match = QPushButton("IMITAR AUDÍFONO")
        self.btn_mode_match.setObjectName("pillToggleBtn")
        self.btn_mode_match.setCheckable(True)
        self.btn_mode_match.clicked.connect(lambda: self._set_autoq_mode("match"))
        mode_row.addWidget(self.btn_mode_match, 1)
        lay.addLayout(mode_row)

        # Stacked Target vs Match panel
        self.autoq_mode_stack = QStackedWidget()

        # Subpage 0: Target curve selector
        tgt_card = QFrame()
        tgt_card.setObjectName("subCard")
        tc = QVBoxLayout(tgt_card)
        tc.setContentsMargins(10, 10, 10, 10)
        tc.setSpacing(8)
        t_lbl = QLabel("CURVA OBJETIVO (8 TARGETS VERIFICADOS)")
        t_lbl.setObjectName("sectionLabel")
        tc.addWidget(t_lbl)
        self.target_combo = QComboBox()
        self.target_combo.setObjectName("nothingCombo")
        tc.addWidget(self.target_combo)
        tc.addStretch(1)
        self.autoq_mode_stack.addWidget(tgt_card)

        # Subpage 1: Imitate any headphone
        match_card = QFrame()
        match_card.setObjectName("subCard")
        mc = QVBoxLayout(match_card)
        mc.setContentsMargins(10, 8, 10, 8)
        mc.setSpacing(6)
        self.match_search_edit = QLineEdit()
        self.match_search_edit.setObjectName("nothingInput")
        self.match_search_edit.setPlaceholderText("Imitar audífono (7Hz Zero 3, Blessing 3, HD 600, AirPods...)")
        self.match_search_edit.textChanged.connect(lambda _: self._match_search_timer.start())
        mc.addWidget(self.match_search_edit)
        self.match_list = QListWidget()
        self.match_list.setObjectName("nothingList")
        self.match_list.setFixedHeight(102)
        self.match_list.itemDoubleClicked.connect(lambda _: self.run_autoq())
        mc.addWidget(self.match_list)
        self.autoq_mode_stack.addWidget(match_card)

        lay.addWidget(self.autoq_mode_stack)

        # Max-Freq ceiling pills
        freq_row = QHBoxLayout()
        freq_row.setSpacing(5)
        fl = QLabel("TECHO:")
        fl.setObjectName("paramLabel")
        freq_row.addWidget(fl)
        self.max_freq_btns: Dict[int, QPushButton] = {}
        for hz, lbl in ((6000, "≤6k"), (8000, "≤8k"), (10000, "≤10k"), (14000, "≤14k"), (20000, "≤20k")):
            b = QPushButton(lbl)
            b.setObjectName("pillToggleBtn")
            b.setCheckable(True)
            b.setChecked(hz == self.max_freq_fit)
            b.clicked.connect(lambda _=False, v=hz: self._set_max_freq(v))
            freq_row.addWidget(b, 1)
            self.max_freq_btns[hz] = b
        lay.addLayout(freq_row)

        # Big Nothing Red CTA
        self.btn_run_autoq = QPushButton("CALCULAR Y APLICAR AUTOQ (10B)")
        self.btn_run_autoq.setObjectName("redActionBtn")
        self.btn_run_autoq.setFixedHeight(36)
        self.btn_run_autoq.clicked.connect(self.run_autoq)
        lay.addWidget(self.btn_run_autoq)

        self.autoq_result_lbl = QLabel("")
        self.autoq_result_lbl.setObjectName("paramValue")
        self.autoq_result_lbl.setWordWrap(True)
        lay.addWidget(self.autoq_result_lbl)

        lay.addStretch(1)
        return page

    def _build_advanced_tab(self) -> QWidget:
        page = QWidget()
        lay = QVBoxLayout(page)
        lay.setContentsMargins(0, 2, 0, 0)
        lay.setSpacing(8)

        # ── Presets Card ─────────────────────────────────────────────────────
        p_card = QFrame()
        p_card.setObjectName("subCard")
        pl = QVBoxLayout(p_card)
        pl.setContentsMargins(10, 8, 10, 8)
        pl.setSpacing(6)

        pl.addWidget(self._section_lbl("PRESETS UNIVERSALES (SINCRONIZADOS CON WIDGET)"))

        self.preset_list = QListWidget()
        self.preset_list.setObjectName("nothingList")
        self.preset_list.setFixedHeight(104)
        self.preset_list.itemDoubleClicked.connect(lambda _: self._load_selected_preset())
        pl.addWidget(self.preset_list)

        p_btns = QHBoxLayout()
        p_btns.setSpacing(5)
        b_load = QPushButton("APLICAR")
        b_load.setObjectName("redActionBtn")
        b_load.clicked.connect(self._load_selected_preset)
        p_btns.addWidget(b_load, 1)
        b_del = QPushButton("ELIMINAR")
        b_del.setObjectName("pillBtn")
        b_del.clicked.connect(self._delete_selected_preset)
        p_btns.addWidget(b_del)
        pl.addLayout(p_btns)

        save_row = QHBoxLayout()
        save_row.setSpacing(5)
        self.new_preset_edit = QLineEdit()
        self.new_preset_edit.setObjectName("nothingInput")
        self.new_preset_edit.setPlaceholderText("Nombre del nuevo preset...")
        save_row.addWidget(self.new_preset_edit, 1)
        b_save = QPushButton("GUARDAR")
        b_save.setObjectName("pillBtn")
        b_save.clicked.connect(self._save_new_preset)
        save_row.addWidget(b_save)
        pl.addLayout(save_row)

        lay.addWidget(p_card)

        # ── EqualizerAPO Import / Export Card ────────────────────────────────
        apo_card = QFrame()
        apo_card.setObjectName("subCard")
        al = QVBoxLayout(apo_card)
        al.setContentsMargins(10, 8, 10, 8)
        al.setSpacing(6)

        al.addWidget(self._section_lbl("EQUALIZER APO (PARAMETRICEQ.TXT)"))
        self.apo_edit = QPlainTextEdit()
        self.apo_edit.setObjectName("nothingTextArea")
        self.apo_edit.setFixedHeight(78)
        self.apo_edit.setPlaceholderText("Pega aquí Filter 1: ON PK Fc 100 Hz Gain 2.5 dB Q 1.2 ...")
        al.addWidget(self.apo_edit)

        apo_btns = QHBoxLayout()
        apo_btns.setSpacing(5)
        b_imp = QPushButton("IMPORTAR APO")
        b_imp.setObjectName("pillBtn")
        b_imp.clicked.connect(self._import_apo)
        apo_btns.addWidget(b_imp, 1)
        b_exp = QPushButton("EXPORTAR Y COPIAR")
        b_exp.setObjectName("pillBtn")
        b_exp.clicked.connect(self._export_apo)
        apo_btns.addWidget(b_exp, 1)
        al.addLayout(apo_btns)

        lay.addWidget(apo_card)

        # ── Bluetooth SPP Firmware Card (shown when supports_spp) ────────────
        self.spp_card = QFrame()
        self.spp_card.setObjectName("subCard")
        sl = QVBoxLayout(self.spp_card)
        sl.setContentsMargins(10, 8, 10, 8)
        sl.setSpacing(6)
        sl.addWidget(self._section_lbl("BLUETOOTH SPP FIRMWARE (MOONDROP)"))

        spp_r1 = QHBoxLayout()
        spp_r1.setSpacing(5)
        for idx, nm in ((0, "FW: POP"), (1, "FW: MON"), (2, "FW: BASS")):
            b = QPushButton(nm)
            b.setObjectName("pillBtn")
            b.clicked.connect(lambda _=False, i=idx: self._bt_spp_preset(i))
            spp_r1.addWidget(b, 1)
        sl.addLayout(spp_r1)

        spp_r2 = QHBoxLayout()
        spp_r2.setSpacing(5)
        b_sr = QPushButton("SPP LEER")
        b_sr.setObjectName("pillBtn")
        b_sr.clicked.connect(lambda: self._bt_spp_rw("read"))
        spp_r2.addWidget(b_sr, 1)
        b_sw = QPushButton("SPP ESCRIBIR")
        b_sw.setObjectName("redActionBtn")
        b_sw.clicked.connect(lambda: self._bt_spp_rw("write"))
        spp_r2.addWidget(b_sw, 1)
        sl.addLayout(spp_r2)

        self.spp_card.setVisible(False)
        lay.addWidget(self.spp_card)

        # ── Hardware & PipeWire Diagnostics Card ─────────────────────────────
        diag_card = QFrame()
        diag_card.setObjectName("subCard")
        dl = QVBoxLayout(diag_card)
        dl.setContentsMargins(10, 8, 10, 8)
        dl.setSpacing(4)
        dl.addWidget(self._section_lbl("DIAGNÓSTICO DE AUDIO Y PIPEWIRE"))
        self.diag_lbl = QLabel("Escaneando...")
        self.diag_lbl.setObjectName("diagText")
        self.diag_lbl.setWordWrap(True)
        dl.addWidget(self.diag_lbl)
        lay.addWidget(diag_card)

        lay.addStretch(1)
        return page

    @staticmethod
    def _section_lbl(txt: str) -> QLabel:
        l = QLabel(txt)
        l.setObjectName("sectionLabel")
        return l

    def _apply_stylesheet(self) -> None:
        self.setStyleSheet(
            f"""
            QWidget {{
                color: {PURE_WHITE};
                font-family: "JetBrains Mono", "FantasqueSansM Nerd Font Mono", "Inter", monospace;
                font-size: 11px;
            }}
            #nothingShell {{
                background-color: {SURFACE};
                border: 1px solid {BORDER};
                border-radius: 26px;
            }}
            #subCard {{
                background-color: {SURFACE_HIGH};
                border: 1px solid {BORDER};
                border-radius: 16px;
            }}
            #tabPillBar {{
                background-color: {OBSIDIAN};
                border: 1px solid {BORDER};
                border-radius: 16px;
            }}
            #tabPillBtn {{
                background: transparent;
                color: {MUTED_GRAY};
                border: none;
                border-radius: 12px;
                padding: 6px 8px;
                font-weight: 700;
                letter-spacing: 1px;
            }}
            #tabPillBtn:checked {{
                background-color: {SURFACE_PILL};
                color: {PURE_WHITE};
                border: 1px solid {BORDER_BRIGHT};
            }}
            #redDot {{
                background-color: {NOTHING_RED};
                border-radius: 4px;
            }}
            #badgePill {{
                background-color: {SURFACE_PILL};
                color: {MUTED_GRAY};
                border: 1px solid {BORDER};
                border-radius: 10px;
                padding: 3px 8px;
                font-size: 10px;
                font-weight: 700;
            }}
            #redPowerBtn {{
                background-color: {NOTHING_RED};
                color: {PURE_WHITE};
                border: none;
                border-radius: 13px;
                font-weight: 800;
                font-size: 10px;
            }}
            #redPowerBtn:hover {{
                background-color: {NOTHING_RED_HOVER};
            }}
            #redPowerBtn[off="true"] {{
                background-color: {SURFACE_PILL};
                color: {MUTED_GRAY};
                border: 1px solid {BORDER};
            }}
            #circlePillBtn {{
                background-color: {SURFACE_PILL};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 14px;
                font-size: 13px;
                font-weight: 700;
            }}
            #circlePillBtn:hover {{
                border-color: {BORDER_BRIGHT};
                background-color: #282B32;
            }}
            #presetPillBtn {{
                background-color: {OBSIDIAN};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 15px;
                padding: 0px 10px;
                font-size: 11px;
                font-weight: 600;
            }}
            #presetPillBtn:hover {{
                border-color: {BORDER_BRIGHT};
            }}
            #expandEqBtn {{
                background-color: {NOTHING_RED};
                color: {PURE_WHITE};
                border: none;
                border-radius: 15px;
                font-size: 11px;
                font-weight: 800;
            }}
            #expandEqBtn:hover {{
                background-color: {NOTHING_RED_HOVER};
            }}
            #redActionBtn {{
                background-color: {NOTHING_RED};
                color: {PURE_WHITE};
                border: none;
                border-radius: 13px;
                min-height: 26px;
                padding: 4px 14px;
                font-weight: 800;
                letter-spacing: 0.6px;
            }}
            #redActionBtn:hover {{
                background-color: {NOTHING_RED_HOVER};
            }}
            #redActionBtn:disabled {{
                background-color: #3A2225;
                color: {MUTED_GRAY};
            }}
            #pillBtn {{
                background-color: {SURFACE_PILL};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 12px;
                min-height: 24px;
                padding: 3px 10px;
                font-weight: 600;
            }}
            #pillBtn:hover {{
                border-color: {BORDER_BRIGHT};
                background-color: #272A31;
            }}
            #pillToggleBtn {{
                background-color: {OBSIDIAN};
                color: {MUTED_GRAY};
                border: 1px solid {BORDER};
                border-radius: 11px;
                min-height: 22px;
                padding: 3px 8px;
                font-weight: 700;
                font-size: 10px;
            }}
            #pillToggleBtn:checked {{
                background-color: {SURFACE_PILL};
                color: {PURE_WHITE};
                border: 1px solid {NOTHING_RED};
            }}
            #brandChipBtn {{
                background-color: {OBSIDIAN};
                color: {MUTED_GRAY};
                border: 1px solid {BORDER};
                border-radius: 10px;
                min-height: 20px;
                padding: 2px 9px;
                font-size: 10px;
            }}
            #brandChipBtn:hover {{
                color: {PURE_WHITE};
                border-color: {NOTHING_RED};
            }}
            #bandChip {{
                background-color: {OBSIDIAN};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 11px;
                padding: 2px 2px;
                font-size: 10px;
                font-weight: 600;
            }}
            #bandChip[selected="true"] {{
                background-color: #241518;
                border: 1px solid {NOTHING_RED};
                color: {PURE_WHITE};
            }}
            QScrollBar:vertical {{
                background: transparent;
                width: 6px;
                margin: 4px 1px 4px 1px;
            }}
            QScrollBar::handle:vertical {{
                background: {BORDER_BRIGHT};
                min-height: 20px;
                border-radius: 3px;
            }}
            QScrollBar::handle:vertical:hover {{
                background: {MUTED_GRAY};
            }}
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
                height: 0px;
            }}
            QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{
                background: transparent;
            }}
            #stepBtn {{
                background-color: {OBSIDIAN};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 11px;
                font-weight: 700;
            }}
            #stepBtn:hover {{
                border-color: {NOTHING_RED};
            }}
            #nothingCombo {{
                background-color: {OBSIDIAN};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 12px;
                padding: 4px 10px;
            }}
            #nothingCombo QAbstractItemView {{
                background-color: {SURFACE};
                color: {PURE_WHITE};
                selection-background-color: {NOTHING_RED};
                border: 1px solid {BORDER};
            }}
            #nothingInput, #nothingTextArea {{
                background-color: {OBSIDIAN};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 12px;
                padding: 5px 10px;
            }}
            #nothingInput:focus, #nothingTextArea:focus {{
                border-color: {NOTHING_RED};
            }}
            #nothingList {{
                background-color: {OBSIDIAN};
                color: {PURE_WHITE};
                border: 1px solid {BORDER};
                border-radius: 12px;
                padding: 3px;
                outline: none;
            }}
            #nothingList::item {{
                padding: 5px 8px;
                border-radius: 8px;
            }}
            #nothingList::item:selected {{
                background-color: #2A161A;
                color: {PURE_WHITE};
                border: 1px solid {NOTHING_RED};
            }}
            #nothingSlider::groove:horizontal {{
                height: 4px;
                background: {OBSIDIAN};
                border-radius: 2px;
            }}
            #nothingSlider::sub-page:horizontal {{
                background: {NOTHING_RED};
                border-radius: 2px;
            }}
            #nothingSlider::handle:horizontal {{
                background: {PURE_WHITE};
                border: 2px solid {NOTHING_RED};
                width: 12px;
                height: 12px;
                margin: -5px 0;
                border-radius: 7px;
            }}
            #sectionLabel {{
                color: {MUTED_GRAY};
                font-size: 10px;
                font-weight: 700;
                letter-spacing: 0.8px;
            }}
            #highlightLabel {{
                color: {PURE_WHITE};
                font-size: 11px;
                font-weight: 700;
            }}
            #paramLabel {{
                color: {MUTED_GRAY};
                font-size: 10px;
                font-weight: 700;
            }}
            #paramValue {{
                color: {PURE_WHITE};
                font-size: 11px;
                font-weight: 700;
            }}
            #diagText {{
                color: {MUTED_GRAY};
                font-size: 10px;
            }}
            #telemetryFooter {{
                color: {MUTED_GRAY};
                font-size: 10px;
                padding-left: 2px;
            }}
            """
        )

    # ── Mode & Tab Management ────────────────────────────────────────────────
    def set_mode(self, full: bool) -> None:
        self.full_mode = full
        self.mode_stack.setCurrentIndex(1 if full else 0)
        if full:
            self.setFixedSize(476, 738)
        else:
            self.setFixedSize(384, 224)
        self._refresh_visuals()

    def switch_tab(self, idx: int) -> None:
        self.tab_stack.setCurrentIndex(idx)
        for i, b in enumerate(self.tab_buttons):
            b.setChecked(i == idx)
        if idx == 1 and not self._source_results:
            self._do_source_search()
            self._do_match_search()

    # ── Drag-to-move Frameless Window (Wayland + X11) ────────────────────────
    def mousePressEvent(self, event: QMouseEvent) -> None:
        if event.button() == Qt.MouseButton.LeftButton:
            wh = self.windowHandle()
            if wh is not None and wh.startSystemMove():
                event.accept()
                return
            self._drag_offset = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
            event.accept()

    def mouseMoveEvent(self, event: QMouseEvent) -> None:
        if self._drag_offset is not None and (event.buttons() & Qt.MouseButton.LeftButton):
            self.move(event.globalPosition().toPoint() - self._drag_offset)
            event.accept()

    def mouseReleaseEvent(self, _event: QMouseEvent) -> None:
        self._drag_offset = None

    # ── Backend Async Dispatch ───────────────────────────────────────────────
    def _run_backend(self, tag: str, args: List[str]) -> None:
        w = BackendWorker(tag, args, self)
        w.finished_json.connect(self._on_backend_done)
        w.finished.connect(lambda: self._cleanup_worker(w))
        self._workers.append(w)
        w.start()

    def _cleanup_worker(self, w: BackendWorker) -> None:
        if w in self._workers:
            self._workers.remove(w)
        w.deleteLater()

    def _set_status(self, msg: str, error: bool = False) -> None:
        self.status_lbl.setText(f"> {msg}")
        self.status_lbl.setStyleSheet(f"color: {'#FF6B72' if error else MUTED_GRAY}; font-size: 10px;")

    def refresh_status(self, force_db: bool = False) -> None:
        args = ["status"]
        if self.active_device:
            args += ["--device-id", self.active_device["id"]]
        if force_db:
            args.append("--refresh-db")
            self.busy = True
            self._set_status("SINCRONIZANDO 8 BASES FR (AUTOEQ + 7 SQUIGLINK)...")
        self._run_backend("status", args)

    def _on_backend_done(self, tag: str, res: Dict[str, Any]) -> None:
        if not res.get("ok"):
            self.busy = False
            self._set_status(str(res.get("error", "ERROR")), error=True)
            return

        if tag == "status":
            self.busy = False
            self.all_devices = res.get("devices") or []
            self.active_device = res.get("device")
            self.pipewire_active = bool(res.get("pipewire_active", False))
            if "dsp_enabled" in res:
                self.dsp_enabled = bool(res["dsp_enabled"])
            self.total_db_count = int(res.get("total_db_count", 0))
            self.active_sources = int(res.get("active_sources", 0))
            self.total_sources = int(res.get("total_sources", 8))

            if res.get("bands"):
                self.bands = res["bands"]
                self.dirty_bands = False
                self.selected_band = min(self.selected_band, len(self.bands) - 1)

            if res.get("auto_source_entry") and not self.selected_source:
                self.selected_source = res["auto_source_entry"]
            if res.get("stock_fr_curve") and not self.stock_fr:
                self.stock_fr = res["stock_fr_curve"]

            if res.get("targets"):
                self.targets = res["targets"]
                self.target_combo.clear()
                for t in self.targets:
                    self.target_combo.addItem(t["label"], t["file"])

            if res.get("presets"):
                self._set_presets(res["presets"])

            self._populate_devices()
            self._rebuild_band_chips()
            self._refresh_visuals()

            if res.get("read_error"):
                self._set_status(res["read_error"], error=True)
            elif self.active_device:
                self._set_status(
                    f"LINK OK // {self.active_device['transport_label']} · "
                    f"{self.total_db_count:,} CURVAS ({self.active_sources}/{self.total_sources})"
                )
            else:
                self._set_status("SIN DISPOSITIVO DE AUDIO DETECTADO")

        elif tag == "sourcefr":
            if res.get("stock_fr_curve"):
                self.stock_fr = res["stock_fr_curve"]
            if res.get("entry"):
                self.selected_source = res["entry"]
                self._set_status(f"SRC: {self.selected_source['name']}")
            self._refresh_visuals()

        elif tag in ("apply", "commit", "flat"):
            self.busy = False
            if res.get("bands"):
                self.bands = res["bands"]
            if "dsp_enabled" in res:
                self.dsp_enabled = bool(res["dsp_enabled"])
            if "pipewire_active" in res:
                self.pipewire_active = bool(res["pipewire_active"])
            if tag in ("commit", "flat") or (self.active_device and self.active_device.get("kind") != "usb_hid"):
                self.dirty_bands = False
            self._rebuild_band_chips()
            self._refresh_visuals()
            self._set_status(str(res.get("message", "SYNCED")).upper())

        elif tag == "srcsearch":
            self._source_results = res.get("results") or []
            self.src_list.clear()
            for r in self._source_results:
                it = QListWidgetItem(r["label"])
                it.setData(Qt.ItemDataRole.UserRole, r)
                self.src_list.addItem(it)

        elif tag == "matchsearch":
            self._match_results = res.get("results") or []
            self.match_list.clear()
            for r in self._match_results:
                it = QListWidgetItem(r["label"])
                it.setData(Qt.ItemDataRole.UserRole, r)
                self.match_list.addItem(it)
            if self.match_list.count() > 0:
                self.match_list.setCurrentRow(0)

        elif tag == "autoeq":
            self.busy = False
            if res.get("bands"):
                self.bands = res["bands"]
                self.selected_band = min(self.selected_band, len(self.bands) - 1)
                self.dirty_bands = True
            self.dsp_enabled = True
            self.target_delta = res.get("target_curve") or []
            if res.get("stock_fr_curve"):
                self.stock_fr = res["stock_fr_curve"]
            self.target_fr = res.get("target_fr_curve") or []
            self.target_label = res.get("target_label") or ""
            self.rmse_text = f"RMSE {res['rmse']:.2f} dB" if "rmse" in res else ""
            self.rig_warning = res.get("warning") or ""

            summary = res.get("message", "AUTOQ CALCULADO")
            if self.rig_warning:
                summary += f"\n⚠ {self.rig_warning}"
            self.autoq_result_lbl.setText(summary)
            self._set_status(res.get("message", "AUTOQ OK").upper())
            self._rebuild_band_chips()
            self._refresh_visuals()
            if self.live_preview and self.active_device:
                self.apply_bands(False)

        elif tag == "preset":
            if res.get("presets"):
                self._set_presets(res["presets"])
            self._set_status(str(res.get("message", "PRESET OK")).upper())

        elif tag == "importapo":
            if res.get("bands"):
                self.bands = res["bands"]
                self.selected_band = 0
                self.dsp_enabled = True
                self.dirty_bands = True
                self._rebuild_band_chips()
                self._refresh_visuals()
                if self.live_preview and self.active_device:
                    self.apply_bands(False)
            self._set_status(str(res.get("message", "APO IMPORTADO")).upper())

        elif tag == "exportapo":
            txt = res.get("text", "")
            self.apo_edit.setPlainText(txt)
            cb = QApplication.clipboard()
            if cb is not None:
                cb.setText(txt)
            self._set_status("PARAMETRICEQ.TXT EXPORTADO Y COPIADO AL PORTAPAPELES")

        elif tag == "btspp":
            self.busy = False
            if res.get("bands"):
                self.bands = res["bands"]
                self._rebuild_band_chips()
                self._refresh_visuals()
            self._set_status(str(res.get("message", "BT SPP OK")).upper())

    # ── UI Sync Helpers ──────────────────────────────────────────────────────
    def _populate_devices(self) -> None:
        self.f_dev_combo.blockSignals(True)
        self.f_dev_combo.clear()
        sel_idx = 0
        for idx, d in enumerate(self.all_devices):
            self.f_dev_combo.addItem(f"{d['display_name']} [{d['transport_label']}]", d["id"])
            if self.active_device and d["id"] == self.active_device["id"]:
                sel_idx = idx
        self.f_dev_combo.setCurrentIndex(sel_idx)
        self.f_dev_combo.blockSignals(False)

    def _set_presets(self, presets: Dict[str, List[Dict[str, Any]]]) -> None:
        self.presets = presets
        self.preset_names = list(presets.keys())
        self.active_preset_idx = min(self.active_preset_idx, max(0, len(self.preset_names) - 1))
        self.preset_list.clear()
        for nm in self.preset_names:
            self.preset_list.addItem(nm)
        if self.preset_names:
            self.w_preset_pill.setText(self.preset_names[self.active_preset_idx])

    def _rebuild_band_chips(self) -> None:
        while self.band_grid.count():
            item = self.band_grid.takeAt(0)
            w = item.widget()
            if w is not None:
                w.deleteLater()
        self.band_chips.clear()

        for i, b in enumerate(self.bands):
            btn = QPushButton()
            btn.setObjectName("bandChip")
            btn.setFixedHeight(36)
            btn.setMinimumWidth(0)
            btn.setSizePolicy(QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Fixed)
            btn.clicked.connect(lambda _=False, idx=i: self.select_band(idx))
            self.band_grid.addWidget(btn, i // 5, i % 5)
            self.band_chips.append(btn)
        for col in range(5):
            self.band_grid.setColumnStretch(col, 1)
        self._update_band_chips_text()

    def _update_band_chips_text(self) -> None:
        for i, (btn, b) in enumerate(zip(self.band_chips, self.bands)):
            f_txt = fmt_freq(float(b["frequency"]))
            g_val = float(b["gain"])
            btn.setText(f"B{i + 1} {f_txt}\n{g_val:+.1f}")
            btn.setProperty("selected", "true" if i == self.selected_band else "false")
            btn.style().unpolish(btn)
            btn.style().polish(btn)

    def _refresh_visuals(self) -> None:
        dev_name = self.active_device["display_name"] if self.active_device else "AUTOQ"
        badge = self.active_device["transport_label"] if self.active_device else "OFFLINE"
        max_b = int(self.active_device["max_bands"]) if self.active_device else 10
        max_g = float(self.active_device["max_gain_db"]) if self.active_device else 12.0

        # Compact widget header
        short_name = dev_name.replace("Moondrop ", "").replace("Ryzen HD Audio Controller ", "AUDIO ").strip()
        self.w_title.setText(short_name[:13] or "AUTOQ")
        self.w_badge.setText(badge)

        # Red dot & power buttons
        dot_css = f"background-color: {NOTHING_RED if (self.active_device and self.dsp_enabled) else DIM_GRAY}; border-radius: 4px;"
        self.w_dot.setStyleSheet(dot_css)
        self.f_dot.setStyleSheet(dot_css)
        for pbtn in (self.w_power_btn, self.f_power_btn):
            pbtn.setText("ON" if self.dsp_enabled else "OFF")
            pbtn.setProperty("off", "false" if self.dsp_enabled else "true")
            pbtn.style().unpolish(pbtn)
            pbtn.style().polish(pbtn)

        # Apply button label
        is_usb = bool(self.active_device and self.active_device.get("kind") == "usb_hid")
        star = " *" if self.dirty_bands else ""
        self.btn_apply.setText(("FLASH" if is_usb else "APLICAR PW") + star)
        self.btn_run_autoq.setText(f"CALCULAR Y APLICAR AUTOQ ({max_b}B)")

        # Catalog & Source display
        if self.total_db_count > 0:
            self.cat_info_lbl.setText(f"CATÁLOGO: {self.total_db_count:,} CURVAS · {self.active_sources}/{self.total_sources} FUENTES")
        if self.selected_source:
            self.src_selected_lbl.setText(f"TU AUDÍFONO: {self.selected_source['name']} [{self.selected_source['source']}]")
        else:
            self.src_selected_lbl.setText("TU AUDÍFONO: Elige abajo entre 14,500+ curvas")

        # SPP card visibility & Diagnostics
        supports_spp = bool(self.active_device and self.active_device.get("supports_spp") and self.active_device.get("mac"))
        self.spp_card.setVisible(supports_spp)
        if self.active_device:
            self.diag_lbl.setText(
                f"Dispositivo: {dev_name}\n"
                f"Transporte: {badge} · Bandas: {max_b} · Rango: ±{max_g:.0f} dB\n"
                f"PipeWire Filter-Chain: {'ACTIVO' if self.pipewire_active else 'INACTIVO'}\n"
                f"Sink/ID: {self.active_device.get('sink_name') or self.active_device['id']}"
            )
        else:
            self.diag_lbl.setText("No hay dispositivos USB HID ni PipeWire detectados.")

        # Update both canvases
        for cv in (self.w_canvas, self.f_canvas):
            cv.set_state(
                self.bands,
                self.selected_band,
                self.dsp_enabled,
                self.overlay_fr,
                self.stock_fr,
                self.target_fr,
                self.target_delta,
                self.target_label,
                self.rmse_text,
                max_g,
            )

        self._update_band_chips_text()
        self._sync_band_sliders()

    def _sync_band_sliders(self) -> None:
        if not self.bands:
            return
        self._updating_controls = True
        idx = max(0, min(self.selected_band, len(self.bands) - 1))
        b = self.bands[idx]
        max_g = int(round((float(self.active_device["max_gain_db"]) if self.active_device else 12.0) * 10))
        self.gain_slider.setRange(-max_g, max_g)

        self.ed_band_title.setText(f"BANDA {idx + 1} / {len(self.bands)}")
        shelves_ok = (not self.active_device) or bool(self.active_device.get("supports_shelves", True))
        for code, btn in self.type_btns.items():
            btn.setChecked(b.get("type", "peaking") == code)
            btn.setEnabled(shelves_ok or code == "peaking")

        freq = max(20.0, min(20000.0, float(b["frequency"])))
        t_log = (math.log10(freq) - math.log10(20.0)) / (math.log10(20000.0) - math.log10(20.0))
        self.freq_slider.setValue(int(round(t_log * 1000)))
        self.freq_val_lbl.setText(f"{int(round(freq))} Hz")

        gain = float(b["gain"])
        self.gain_slider.setValue(int(round(gain * 10)))
        self.gain_val_lbl.setText(f"{gain:+.1f} dB")

        q_val = max(0.1, min(10.0, float(b["q"])))
        self.q_slider.setValue(int(round(q_val * 100)))
        self.q_val_lbl.setText(f"Q {q_val:.2f}")
        self._updating_controls = False

    # ── EQ Control Actions ───────────────────────────────────────────────────
    def select_band(self, idx: int) -> None:
        if 0 <= idx < len(self.bands):
            self.selected_band = idx
            self._refresh_visuals()

    def _update_band_field(self, field: str, value: Any) -> None:
        if not self.bands:
            return
        idx = max(0, min(self.selected_band, len(self.bands) - 1))
        self.bands[idx][field] = value
        self.dirty_bands = True
        self._refresh_visuals()
        if self.live_preview and self.active_device and self.dsp_enabled:
            self._preview_timer.start()

    def _set_band_type(self, code: str) -> None:
        self._update_band_field("type", code)

    def _on_freq_slider(self, val: int) -> None:
        if self._updating_controls:
            return
        lo, hi = math.log10(20.0), math.log10(20000.0)
        freq = round(10.0 ** (lo + (val / 1000.0) * (hi - lo)))
        self._update_band_field("frequency", float(freq))

    def _on_gain_slider(self, val: int) -> None:
        if self._updating_controls:
            return
        self._update_band_field("gain", round(val / 10.0, 1))

    def _on_q_slider(self, val: int) -> None:
        if self._updating_controls:
            return
        self._update_band_field("q", round(val / 100.0, 2))

    def _step_freq(self, direction: int) -> None:
        cur = float(self.bands[self.selected_band]["frequency"])
        step = max(5.0, round(cur * 0.05))
        self._update_band_field("frequency", float(max(20.0, min(20000.0, cur + direction * step))))

    def _step_gain(self, delta: float) -> None:
        max_g = float(self.active_device["max_gain_db"]) if self.active_device else 12.0
        cur = float(self.bands[self.selected_band]["gain"])
        self._update_band_field("gain", round(max(-max_g, min(max_g, cur + delta)), 1))

    def _step_q(self, delta: float) -> None:
        cur = float(self.bands[self.selected_band]["q"])
        self._update_band_field("q", round(max(0.1, min(10.0, cur + delta)), 2))

    def _on_canvas_drag(self, idx: int, freq: float, gain: float) -> None:
        self.selected_band = idx
        self.bands[idx]["frequency"] = freq
        self.bands[idx]["gain"] = gain
        self.dirty_bands = True
        self._refresh_visuals()
        if self.live_preview and self.active_device and self.dsp_enabled:
            self._preview_timer.start()

    def _on_canvas_q(self, idx: int, new_q: float) -> None:
        self.selected_band = idx
        self._update_band_field("q", new_q)

    def _toggle_fr_mode(self) -> None:
        self.overlay_fr = self.btn_fr_mode.isChecked()
        self.btn_fr_mode.setText("FR" if self.overlay_fr else "ΔEQ")
        self._refresh_visuals()

    def _toggle_live(self) -> None:
        self.live_preview = self.btn_live.isChecked()

    def toggle_dsp(self) -> None:
        self.dsp_enabled = not self.dsp_enabled
        self._refresh_visuals()
        if self.active_device:
            self.apply_bands(False)

    def apply_bands(self, commit: bool) -> None:
        if not self.active_device:
            self._set_status("SIN DISPOSITIVO DE AUDIO", error=True)
            return
        args = [
            "apply",
            "--device-id",
            self.active_device["id"],
            "--bands-b64",
            to_b64(json.dumps(self.bands)),
        ]
        if commit:
            args.append("--commit")
            self.busy = True
        if not self.dsp_enabled:
            args.append("--bypass")
        self._run_backend("commit" if commit else "apply", args)

    def set_flat(self) -> None:
        self.target_delta = []
        self.target_fr = []
        self.target_label = ""
        self.rmse_text = ""
        self.dsp_enabled = True
        args = ["flat"]
        if self.active_device:
            args += ["--device-id", self.active_device["id"]]
        self._run_backend("flat", args)

    def _on_device_changed(self, idx: int) -> None:
        if 0 <= idx < len(self.all_devices):
            self.active_device = self.all_devices[idx]
            self.stock_fr = []
            self.selected_source = None
            self.refresh_status(False)

    # ── AutoQ Tab Actions ────────────────────────────────────────────────────
    def _do_source_search(self) -> None:
        q = self.src_search_edit.text().strip()
        self._run_backend("srcsearch", ["search", "--q-b64", to_b64(q), "--limit", "40"])

    def _do_match_search(self) -> None:
        q = self.match_search_edit.text().strip()
        self._run_backend("matchsearch", ["search", "--q-b64", to_b64(q), "--limit", "40"])

    def _on_source_picked(self, item: QListWidgetItem) -> None:
        entry = item.data(Qt.ItemDataRole.UserRole)
        if not entry:
            return
        self.selected_source = entry
        self._refresh_visuals()
        self._run_backend("sourcefr", ["source-fr", "--src-b64", to_b64(entry["rel_path"])])

    def _set_autoq_mode(self, mode: str) -> None:
        self.autoq_mode = mode
        self.btn_mode_target.setChecked(mode == "target")
        self.btn_mode_match.setChecked(mode == "match")
        self.autoq_mode_stack.setCurrentIndex(0 if mode == "target" else 1)
        if mode == "match" and not self._match_results:
            self._do_match_search()

    def _set_max_freq(self, hz: int) -> None:
        self.max_freq_fit = hz
        for k, b in self.max_freq_btns.items():
            b.setChecked(k == hz)

    def run_autoq(self) -> None:
        if not self.selected_source:
            self._set_status("ELIGE PRIMERO TU AUDÍFONO (ORIGEN)", error=True)
            self.autoq_result_lbl.setText("Selecciona tu audífono en la lista superior.")
            return
        max_b = int(self.active_device["max_bands"]) if self.active_device else 10
        max_g = float(self.active_device["max_gain_db"]) if self.active_device else 12.0
        shelves = (not self.active_device) or bool(self.active_device.get("supports_shelves", True))

        common = [
            "--src-b64",
            to_b64(self.selected_source["rel_path"]),
            "--max-freq",
            str(self.max_freq_fit),
            "--bands",
            str(max_b),
            "--max-gain",
            str(max_g),
        ]
        if not shelves:
            common.append("--no-shelves")

        self.busy = True
        if self.autoq_mode == "target":
            t_file = self.target_combo.currentData() or ""
            self._set_status(f"CALCULANDO TARGET ({max_b}B)...")
            self._run_backend("autoeq", ["autoeq-target", "--target-b64", to_b64(str(t_file)), *common])
        else:
            cur_item = self.match_list.currentItem()
            if not cur_item:
                self.busy = False
                self._set_status("ELIGE UN AUDÍFONO A IMITAR", error=True)
                return
            tgt = cur_item.data(Qt.ItemDataRole.UserRole)
            self._set_status(f"IMITANDO {tgt['name'].upper()} ({max_b}B)...")
            self._run_backend("autoeq", ["autoeq-match", "--tgt-b64", to_b64(tgt["rel_path"]), *common])

    # ── Presets, APO & BT SPP Actions ────────────────────────────────────────
    def cycle_preset(self, delta: int) -> None:
        if not self.preset_names:
            return
        self.active_preset_idx = (self.active_preset_idx + delta) % len(self.preset_names)
        name = self.preset_names[self.active_preset_idx]
        self._apply_preset_by_name(name)

    def _apply_preset_by_name(self, name: str) -> None:
        raw = self.presets.get(name)
        if not raw:
            return
        self.w_preset_pill.setText(name)
        self.target_delta = []
        self.target_fr = []
        self.target_label = ""
        self.rmse_text = ""
        self.bands = self._fit_bands_to_device(raw)
        self.selected_band = 0
        self.dsp_enabled = True
        self._rebuild_band_chips()
        self._refresh_visuals()
        if self.active_device:
            self.apply_bands(False)

    def _load_selected_preset(self) -> None:
        it = self.preset_list.currentItem()
        if it is None:
            return
        name = it.text()
        if name in self.preset_names:
            self.active_preset_idx = self.preset_names.index(name)
        self._apply_preset_by_name(name)

    def _save_new_preset(self) -> None:
        name = self.new_preset_edit.text().strip()
        if not name:
            return
        self._run_backend(
            "preset",
            ["preset-save", "--name-b64", to_b64(name), "--bands-b64", to_b64(json.dumps(self.bands))],
        )
        self.new_preset_edit.clear()

    def _delete_selected_preset(self) -> None:
        it = self.preset_list.currentItem()
        if it is None:
            return
        self._run_backend("preset", ["preset-delete", "--name-b64", to_b64(it.text())])

    def _import_apo(self) -> None:
        txt = self.apo_edit.toPlainText().strip()
        if not txt:
            return
        max_b = int(self.active_device["max_bands"]) if self.active_device else 10
        self._run_backend("importapo", ["import-apo", "--text-b64", to_b64(txt), "--bands", str(max_b)])

    def _export_apo(self) -> None:
        self._run_backend("exportapo", ["export-apo", "--bands-b64", to_b64(json.dumps(self.bands))])

    def _bt_spp_preset(self, idx: int) -> None:
        if not self.active_device or not self.active_device.get("mac"):
            return
        self._run_backend("btspp", ["bt-spp", "--mac", self.active_device["mac"], "--action", "preset", "--preset-idx", str(idx)])

    def _bt_spp_rw(self, action: str) -> None:
        if not self.active_device or not self.active_device.get("mac"):
            return
        args = ["bt-spp", "--mac", self.active_device["mac"], "--action", action]
        if action == "write":
            args += ["--bands-b64", to_b64(json.dumps(self.bands))]
        self.busy = True
        self._run_backend("btspp", args)

    def wait_workers(self) -> None:
        for w in list(self._workers):
            if w.isRunning():
                w.wait(3000)

    def closeEvent(self, event) -> None:
        self.wait_workers()
        super().closeEvent(event)

    # ── Single-Instance IPC Handling ─────────────────────────────────────────
    def _handle_ipc(self, cmd: str) -> None:
        if cmd == "toggle":
            if self.isVisible():
                self.hide()
            else:
                self.show()
                self.raise_()
                self.activateWindow()
        elif cmd == "widget":
            self.set_mode(False)
            self.show()
            self.raise_()
            self.activateWindow()
        elif cmd == "full":
            self.set_mode(True)
            self.show()
            self.raise_()
            self.activateWindow()


# ── Single-Instance Unix Socket IPC ──────────────────────────────────────────
def send_ipc_if_running(cmd: str) -> bool:
    if not SOCK_PATH.exists():
        return False
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(0.6)
            s.connect(str(SOCK_PATH))
            s.sendall(cmd.encode("utf-8"))
        return True
    except OSError:
        try:
            SOCK_PATH.unlink(missing_ok=True)
        except OSError:
            pass
        return False


def start_ipc_server(win: AutoQNothingWindow) -> None:
    try:
        SOCK_PATH.unlink(missing_ok=True)
    except OSError:
        pass
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(str(SOCK_PATH))
    srv.listen(4)

    def _loop() -> None:
        while True:
            try:
                conn, _ = srv.accept()
                with conn:
                    data = conn.recv(64).decode("utf-8", errors="ignore").strip()
                    if data:
                        win.ipc_command.emit(data)
            except OSError:
                break

    t = threading.Thread(target=_loop, daemon=True)
    t.start()


def cmd_waybar_status() -> int:
    try:
        raw = subprocess.check_output(
            [sys.executable, str(BACKEND_SCRIPT), "status"],
            text=True,
            timeout=10,
        )
        d = json.loads(raw)
        dev = d.get("device")
        if dev:
            badge = dev.get("transport_label", "EQ")
            on = bool(d.get("dsp_enabled", True))
            dot = "●" if on else "○"
            out = {
                "text": f"{dot} {badge}",
                "tooltip": f"AutoQ — {dev.get('display_name', '')}\n{d.get('total_db_count', 0):,} curvas ({d.get('active_sources', 0)}/{d.get('total_sources', 8)} fuentes)\nClick: abrir widget Nothing OS",
                "class": "active" if on else "bypass",
            }
        else:
            out = {"text": "○ AUTOQ", "tooltip": "AutoQ — Sin dispositivo", "class": "offline"}
    except Exception as exc:
        out = {"text": "○ AUTOQ", "tooltip": str(exc), "class": "offline"}
    print(json.dumps(out, ensure_ascii=False))
    return 0


def render_screenshot_demo(out_dir: Path) -> int:
    out_dir.mkdir(parents=True, exist_ok=True)
    app = QApplication(sys.argv)
    app.setDesktopFileName("autoq-widget")
    win = AutoQNothingWindow(start_full=False)

    # Populate realistic demo data from live backend synchronously
    raw = subprocess.check_output([sys.executable, str(BACKEND_SCRIPT), "status"], text=True)
    st = json.loads(raw)
    win._on_backend_done("status", st)

    # Load real Golden Ages 2 -> 7Hz x Crinacle Zero 3 10-band fit for rich curve visualization
    s_ga = json.loads(subprocess.check_output([sys.executable, str(BACKEND_SCRIPT), "search", "--q", "golden ages 2", "--limit", "1"], text=True))
    s_z3 = json.loads(subprocess.check_output([sys.executable, str(BACKEND_SCRIPT), "search", "--q", "7hz x crinacle zero 3", "--limit", "1"], text=True))
    if s_ga.get("results") and s_z3.get("results"):
        win.selected_source = s_ga["results"][0]
        m = json.loads(
            subprocess.check_output(
                [
                    sys.executable,
                    str(BACKEND_SCRIPT),
                    "autoeq-match",
                    "--src-b64",
                    to_b64(s_ga["results"][0]["rel_path"]),
                    "--tgt-b64",
                    to_b64(s_z3["results"][0]["rel_path"]),
                    "--bands",
                    "10",
                    "--max-freq",
                    "10000",
                ],
                text=True,
            )
        )
        win.live_preview = False
        win._on_backend_done("autoeq", m)
        win.w_preset_pill.setText("7hz zero 3")

    # 1) Compact Nothing OS Widget
    win.set_mode(False)
    win.show()
    app.processEvents()
    win.grab().save(str(out_dir / "autoq_nothing_widget.png"))

    # 2) Expanded Full App - EQ Tab
    win.set_mode(True)
    win.switch_tab(0)
    app.processEvents()
    win.grab().save(str(out_dir / "autoq_nothing_full_eq.png"))

    # 3) Expanded Full App - AUTOQ Tab
    win._on_backend_done("srcsearch", s_ga)
    win._on_backend_done("matchsearch", s_z3)
    win.switch_tab(1)
    app.processEvents()
    win.grab().save(str(out_dir / "autoq_nothing_full_autoq.png"))

    # 4) Expanded Full App - AVANZADO Tab
    win.switch_tab(2)
    app.processEvents()
    win.grab().save(str(out_dir / "autoq_nothing_full_adv.png"))

    win.wait_workers()
    print("Rendered demo PNGs to", out_dir)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="AutoQ — Nothing OS EQ Widget & App for Hyprland")
    parser.add_argument("--widget", action="store_true", help="Iniciar en modo widget compacto Nothing OS (default)")
    parser.add_argument("--full", action="store_true", help="Iniciar en modo app completa de 3 pestañas")
    parser.add_argument("--toggle", action="store_true", help="Mostrar/ocultar instancia activa (ideal para bind SUPER+E)")
    parser.add_argument("--status-waybar", action="store_true", help="Salida JSON de una línea para Waybar")
    parser.add_argument("--screenshot-demo", default="", help="Renderizar capturas PNG offscreen en carpeta indicada")
    args = parser.parse_args()

    if args.status_waybar:
        return cmd_waybar_status()
    if args.screenshot_demo:
        return render_screenshot_demo(Path(args.screenshot_demo))

    ipc_cmd = "toggle" if args.toggle else ("full" if args.full else "widget")
    if send_ipc_if_running(ipc_cmd):
        return 0

    app = QApplication(sys.argv)
    app.setApplicationName("autoq-widget")
    app.setDesktopFileName("autoq-widget")

    win = AutoQNothingWindow(start_full=args.full)
    start_ipc_server(win)
    win.show()
    win.refresh_status(False)
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
