#!/usr/bin/env python3
"""Moondrop DSP Desktop Widget & AutoEQ Studio for Linux (KDE / Qt6)."""

from __future__ import annotations

import json
import math
import os
import sys
from pathlib import Path
from typing import Callable, Dict, List, Optional, Sequence, Tuple

import numpy as np
from PySide6 import QtCore, QtGui, QtWidgets

import autoeq_db as db
from biquad import (
    FilterBand,
    MAX_FREQ_HZ,
    MAX_GAIN_DB,
    MAX_Q,
    MIN_FREQ_HZ,
    MIN_Q,
    band_response_db,
    combined_response_db,
)
import chu2_hid

CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "chu2-dsp-widget"
PRESETS_FILE = CONFIG_DIR / "presets.json"
ICON_PATH = Path(__file__).resolve().parent / "chu2_icon.svg"

MAX_SUPPORTED_BANDS = 8

BAND_COLORS = [
    QtGui.QColor("#38bdf8"),  # Sky
    QtGui.QColor("#34d399"),  # Emerald
    QtGui.QColor("#a78bfa"),  # Violet
    QtGui.QColor("#fbbf24"),  # Amber
    QtGui.QColor("#f472b6"),  # Pink
    QtGui.QColor("#fb7185"),  # Rose
    QtGui.QColor("#2dd4bf"),  # Teal
    QtGui.QColor("#c084fc"),  # Purple
]

FACTORY_BANDS = [
    FilterBand("peaking", 20.0, -1.1, 1.370),
    FilterBand("peaking", 63.0, 2.6, 0.800),
    FilterBand("peaking", 2200.0, 3.0, 0.200),
    FilterBand("peaking", 6500.0, 3.0, 2.800),
    FilterBand("peq" if False else "peaking", 8000.0, 0.0, 0.707),
]

FLAT_BANDS = [FilterBand("peaking", f, 0.0, 1.0) for f in (60.0, 250.0, 600.0, 1500.0, 3500.0, 6000.0, 9000.0, 12000.0)]


def _default_presets() -> Dict[str, List[dict]]:
    return {
        "Plano / Stock sin DSP (0 dB)": [vars(b) for b in FLAT_BANDS[:5]],
        "Chu 2 DSP (Fábrica)": [vars(b) for b in FACTORY_BANDS],
    }


def load_user_presets() -> Dict[str, List[FilterBand]]:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    if not PRESETS_FILE.exists():
        raw = _default_presets()
        PRESETS_FILE.write_text(json.dumps(raw, indent=2, ensure_ascii=False), encoding="utf-8")
    else:
        try:
            raw = json.loads(PRESETS_FILE.read_text(encoding="utf-8"))
        except Exception:
            raw = _default_presets()
    out: Dict[str, List[FilterBand]] = {}
    for name, items in raw.items():
        bands = [FilterBand(**item).clamped() for item in items[:MAX_SUPPORTED_BANDS]]
        out[name] = bands
    return out


def save_user_presets(presets: Dict[str, List[FilterBand]]) -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    raw = {name: [vars(b.clamped()) for b in bands[:MAX_SUPPORTED_BANDS]] for name, bands in presets.items()}
    PRESETS_FILE.write_text(json.dumps(raw, indent=2, ensure_ascii=False), encoding="utf-8")


class _WorkerSignals(QtCore.QObject):
    finished = QtCore.Signal(object)
    failed = QtCore.Signal(str)


class _RunnableTask(QtCore.QRunnable):
    """QRunnable executing a blocking HID or HTTP task in QThreadPool."""

    def __init__(self, fn: Callable[[], object]) -> None:
        super().__init__()
        self.signals = _WorkerSignals()
        self._fn = fn

    def run(self) -> None:
        try:
            res = self._fn()
            self.signals.finished.emit(res)
        except Exception as exc:
            self.signals.failed.emit(str(exc))


class PEQGraphWidget(QtWidgets.QWidget):
    """Interactive log-frequency PEQ plot with draggable nodes and AutoEQ overlay."""

    bandDragged = QtCore.Signal(int, float, float)  # index, freq, gain
    bandQScrolled = QtCore.Signal(int, float)       # index, new_q
    bandSelected = QtCore.Signal(int)

    def __init__(self, parent: Optional[QtWidgets.QWidget] = None) -> None:
        super().__init__(parent)
        self.setMinimumHeight(205)
        self.setMouseTracking(True)
        self._bands: List[FilterBand] = [FilterBand(**vars(b)) for b in FLAT_BANDS[:5]]
        self._freqs = np.geomspace(20.0, 20000.0, 260)
        self._target_freqs: Optional[np.ndarray] = None
        self._target_db: Optional[np.ndarray] = None
        self._selected_idx: int = 0
        self._drag_idx: Optional[int] = None
        self._hover_idx: Optional[int] = None
        self._db_min = -15.0
        self._db_max = 15.0

    def set_bands(self, bands: Sequence[FilterBand]) -> None:
        self._bands = [b.clamped() for b in bands[:MAX_SUPPORTED_BANDS]]
        self.update()

    def set_selected(self, idx: int) -> None:
        self._selected_idx = idx
        self.update()

    def set_target_overlay(self, freqs: Optional[np.ndarray], target_db: Optional[np.ndarray]) -> None:
        self._target_freqs = freqs
        self._target_db = target_db
        self.update()

    def _plot_rect(self) -> QtCore.QRectF:
        return QtCore.QRectF(42.0, 14.0, max(50.0, self.width() - 56.0), max(50.0, self.height() - 38.0))

    def _freq_to_x(self, freq: float, rect: QtCore.QRectF) -> float:
        lo, hi = math.log10(20.0), math.log10(20000.0)
        val = (math.log10(max(20.0, min(20000.0, freq))) - lo) / (hi - lo)
        return rect.left() + val * rect.width()

    def _x_to_freq(self, x: float, rect: QtCore.QRectF) -> float:
        lo, hi = math.log10(20.0), math.log10(20000.0)
        t = min(1.0, max(0.0, (x - rect.left()) / rect.width()))
        return 10.0 ** (lo + t * (hi - lo))

    def _db_to_y(self, db_val: float, rect: QtCore.QRectF) -> float:
        t = (db_val - self._db_min) / (self._db_max - self._db_min)
        return rect.bottom() - t * rect.height()

    def _y_to_db(self, y: float, rect: QtCore.QRectF) -> float:
        t = (rect.bottom() - y) / rect.height()
        db_val = self._db_min + t * (self._db_max - self._db_min)
        return min(MAX_GAIN_DB, max(-MAX_GAIN_DB, db_val))

    def _hit_node(self, pos: QtCore.QPointF) -> Optional[int]:
        rect = self._plot_rect()
        best_i: Optional[int] = None
        best_d2 = 15.0 ** 2
        for i, b in enumerate(self._bands):
            nx = self._freq_to_x(b.frequency, rect)
            ny = self._db_to_y(b.gain, rect)
            d2 = (pos.x() - nx) ** 2 + (pos.y() - ny) ** 2
            if d2 < best_d2:
                best_d2 = d2
                best_i = i
        return best_i

    def mousePressEvent(self, event: QtGui.QMouseEvent) -> None:
        if event.button() == QtCore.Qt.MouseButton.LeftButton:
            hit = self._hit_node(event.position())
            if hit is not None:
                self._drag_idx = hit
                self._selected_idx = hit
                self.bandSelected.emit(hit)
                self.update()
                return
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QtGui.QMouseEvent) -> None:
        rect = self._plot_rect()
        if self._drag_idx is not None:
            f = round(self._x_to_freq(event.position().x(), rect))
            g = round(self._y_to_db(event.position().y(), rect), 1)
            self.bandDragged.emit(self._drag_idx, float(f), float(g))
            return
        hit = self._hit_node(event.position())
        if hit != self._hover_idx:
            self._hover_idx = hit
            self.setCursor(QtCore.Qt.CursorShape.PointingHandCursor if hit is not None else QtCore.Qt.CursorShape.ArrowCursor)
            self.update()
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QtGui.QMouseEvent) -> None:
        if self._drag_idx is not None and event.button() == QtCore.Qt.MouseButton.LeftButton:
            self._drag_idx = None
            self.update()
            return
        super().mouseReleaseEvent(event)

    def wheelEvent(self, event: QtGui.QWheelEvent) -> None:
        idx = self._drag_idx if self._drag_idx is not None else self._hover_idx
        if idx is not None and idx < len(self._bands):
            delta = event.angleDelta().y() / 120.0
            cur_q = self._bands[idx].q
            new_q = round(min(MAX_Q, max(MIN_Q, cur_q * (1.12 ** delta))), 3)
            self.bandQScrolled.emit(idx, new_q)
            event.accept()
            return
        super().wheelEvent(event)

    def paintEvent(self, event: QtGui.QPaintEvent) -> None:
        p = QtGui.QPainter(self)
        p.setRenderHint(QtGui.QPainter.RenderHint.Antialiasing, True)
        rect = self._plot_rect()

        p.setPen(QtGui.QPen(QtGui.QColor("#2c3448"), 1))
        p.setBrush(QtGui.QColor("#111520"))
        p.drawRoundedRect(self.rect().adjusted(1, 1, -1, -1), 10, 10)

        grid_freqs = [20, 30, 50, 100, 200, 300, 500, 1000, 2000, 3000, 5000, 10000, 20000]
        label_freqs = {20: "20", 50: "50", 100: "100", 200: "200", 500: "500", 1000: "1k", 2000: "2k", 5000: "5k", 10000: "10k", 20000: "20k"}
        font = p.font()
        font.setPointSize(8)
        p.setFont(font)

        for gf in grid_freqs:
            x = self._freq_to_x(gf, rect)
            p.setPen(QtGui.QPen(QtGui.QColor("#1f2738"), 1))
            p.drawLine(QtCore.QPointF(x, rect.top()), QtCore.QPointF(x, rect.bottom()))
            if gf in label_freqs:
                p.setPen(QtGui.QColor("#7f8ea8"))
                p.drawText(QtCore.QRectF(x - 16, rect.bottom() + 3, 32, 16), QtCore.Qt.AlignmentFlag.AlignCenter, label_freqs[gf])

        for gdb in (-12, -6, 0, 6, 12):
            y = self._db_to_y(gdb, rect)
            is_zero = (gdb == 0)
            p.setPen(QtGui.QPen(QtGui.QColor("#394661" if is_zero else "#1f2738"), 1.2 if is_zero else 1.0))
            p.drawLine(QtCore.QPointF(rect.left(), y), QtCore.QPointF(rect.right(), y))
            p.setPen(QtGui.QColor("#94a3b8" if is_zero else "#7f8ea8"))
            p.drawText(QtCore.QRectF(4, y - 8, 34, 16), QtCore.Qt.AlignmentFlag.AlignRight | QtCore.Qt.AlignmentFlag.AlignVCenter, f"{gdb:+d}" if gdb else "0")

        p.save()
        p.setClipRect(rect)

        if 0 <= self._selected_idx < len(self._bands):
            sel_b = self._bands[self._selected_idx]
            if abs(sel_b.gain) >= 0.05:
                sel_resp = band_response_db(sel_b, self._freqs)
                sel_path = QtGui.QPainterPath()
                for k, (f, d) in enumerate(zip(self._freqs, sel_resp)):
                    pt = QtCore.QPointF(self._freq_to_x(f, rect), self._db_to_y(d, rect))
                    if k == 0:
                        sel_path.moveTo(pt)
                    else:
                        sel_path.lineTo(pt)
                col = QtGui.QColor(BAND_COLORS[self._selected_idx % len(BAND_COLORS)])
                col.setAlpha(110)
                p.setPen(QtGui.QPen(col, 1.3, QtCore.Qt.PenStyle.DashDotLine))
                p.drawPath(sel_path)

        if self._target_freqs is not None and self._target_db is not None and len(self._target_freqs) > 1:
            t_path = QtGui.QPainterPath()
            for k, (f, d) in enumerate(zip(self._target_freqs, self._target_db)):
                pt = QtCore.QPointF(self._freq_to_x(float(f), rect), self._db_to_y(float(d), rect))
                if k == 0:
                    t_path.moveTo(pt)
                else:
                    t_path.lineTo(pt)
            p.setPen(QtGui.QPen(QtGui.QColor("#fb923c"), 1.8, QtCore.Qt.PenStyle.DashLine))
            p.drawPath(t_path)

        total_db = combined_response_db(self._bands, self._freqs)
        curve_path = QtGui.QPainterPath()
        fill_path = QtGui.QPainterPath()
        zero_y = self._db_to_y(0.0, rect)
        for k, (f, d) in enumerate(zip(self._freqs, total_db)):
            pt = QtCore.QPointF(self._freq_to_x(float(f), rect), self._db_to_y(float(d), rect))
            if k == 0:
                curve_path.moveTo(pt)
                fill_path.moveTo(QtCore.QPointF(pt.x(), zero_y))
                fill_path.lineTo(pt)
            else:
                curve_path.lineTo(pt)
                fill_path.lineTo(pt)
        fill_path.lineTo(QtCore.QPointF(self._freq_to_x(float(self._freqs[-1]), rect), zero_y))
        fill_path.closeSubpath()

        grad = QtGui.QLinearGradient(0, rect.top(), 0, rect.bottom())
        grad.setColorAt(0.0, QtGui.QColor(56, 189, 248, 70))
        grad.setColorAt(0.5, QtGui.QColor(56, 189, 248, 18))
        grad.setColorAt(1.0, QtGui.QColor(129, 140, 248, 65))
        p.fillPath(fill_path, grad)
        p.setPen(QtGui.QPen(QtGui.QColor("#38bdf8"), 2.4))
        p.drawPath(curve_path)
        p.restore()

        bold_font = p.font()
        bold_font.setPointSize(8)
        bold_font.setBold(True)
        p.setFont(bold_font)
        for i, b in enumerate(self._bands):
            nx = self._freq_to_x(b.frequency, rect)
            ny = self._db_to_y(b.gain, rect)
            col = BAND_COLORS[i % len(BAND_COLORS)]
            is_active = (i == self._selected_idx or i == self._hover_idx or i == self._drag_idx)
            r = 9.0 if is_active else 7.0
            p.setPen(QtGui.QPen(QtGui.QColor("#ffffff" if is_active else "#0f172a"), 2.0 if is_active else 1.5))
            p.setBrush(col)
            p.drawEllipse(QtCore.QPointF(nx, ny), r, r)
            p.setPen(QtGui.QColor("#090d16"))
            p.drawText(QtCore.QRectF(nx - r, ny - r, 2 * r, 2 * r), QtCore.Qt.AlignmentFlag.AlignCenter, str(i + 1))


class BandRowWidget(QtWidgets.QWidget):
    """Single row of controls for one hardware PEQ band."""

    changed = QtCore.Signal(int)

    def __init__(self, index: int, parent: Optional[QtWidgets.QWidget] = None) -> None:
        super().__init__(parent)
        self.index = index
        self._blocking = False

        layout = QtWidgets.QHBoxLayout(self)
        layout.setContentsMargins(6, 2, 6, 2)
        layout.setSpacing(6)

        badge = QtWidgets.QLabel(f"B{index + 1}")
        badge.setFixedWidth(26)
        badge.setAlignment(QtCore.Qt.AlignmentFlag.AlignCenter)
        col = BAND_COLORS[index % len(BAND_COLORS)].name()
        badge.setStyleSheet(
            f"background:{col}; color:#090d16; font-weight:700; border-radius:6px; padding:2px 4px;"
        )
        layout.addWidget(badge)

        self.type_combo = QtWidgets.QComboBox()
        self.type_combo.addItem("Peaking", "peaking")
        self.type_combo.addItem("Low Shelf", "low_shelf")
        self.type_combo.addItem("High Shelf", "high_shelf")
        self.type_combo.setFixedWidth(96)
        layout.addWidget(self.type_combo)

        self.freq_spin = QtWidgets.QSpinBox()
        self.freq_spin.setRange(int(MIN_FREQ_HZ), int(MAX_FREQ_HZ))
        self.freq_spin.setSuffix(" Hz")
        self.freq_spin.setFixedWidth(88)
        layout.addWidget(self.freq_spin)

        self.gain_slider = QtWidgets.QSlider(QtCore.Qt.Orientation.Horizontal)
        self.gain_slider.setRange(-120, 120)
        self.gain_slider.setMinimumWidth(90)
        layout.addWidget(self.gain_slider, 1)

        self.gain_spin = QtWidgets.QDoubleSpinBox()
        self.gain_spin.setRange(-MAX_GAIN_DB, MAX_GAIN_DB)
        self.gain_spin.setDecimals(1)
        self.gain_spin.setSingleStep(0.2)
        self.gain_spin.setSuffix(" dB")
        self.gain_spin.setFixedWidth(82)
        layout.addWidget(self.gain_spin)

        self.q_spin = QtWidgets.QDoubleSpinBox()
        self.q_spin.setRange(MIN_Q, MAX_Q)
        self.q_spin.setDecimals(2)
        self.q_spin.setSingleStep(0.1)
        self.q_spin.setPrefix("Q ")
        self.q_spin.setFixedWidth(74)
        layout.addWidget(self.q_spin)

        reset_btn = QtWidgets.QToolButton()
        reset_btn.setText("0")
        reset_btn.setToolTip("Poner banda a 0.0 dB")
        reset_btn.setFixedWidth(26)
        reset_btn.clicked.connect(lambda: self.gain_spin.setValue(0.0))
        layout.addWidget(reset_btn)

        self.type_combo.currentIndexChanged.connect(self._on_edit)
        self.freq_spin.valueChanged.connect(self._on_edit)
        self.gain_slider.valueChanged.connect(self._on_slider)
        self.gain_spin.valueChanged.connect(self._on_spin_gain)
        self.q_spin.valueChanged.connect(self._on_edit)

    def set_constraints(self, max_gain_db: float, supports_shelves: bool) -> None:
        self._blocking = True
        limit = int(round(max_gain_db * 10))
        self.gain_slider.setRange(-limit, limit)
        self.gain_spin.setRange(-max_gain_db, max_gain_db)
        if not supports_shelves:
            self.type_combo.setCurrentIndex(0)
        self.type_combo.setEnabled(supports_shelves)
        self._blocking = False

    def _on_slider(self, val: int) -> None:
        if self._blocking:
            return
        self._blocking = True
        self.gain_spin.setValue(val / 10.0)
        self._blocking = False
        self.changed.emit(self.index)

    def _on_spin_gain(self, val: float) -> None:
        if self._blocking:
            return
        self._blocking = True
        self.gain_slider.setValue(round(val * 10))
        self._blocking = False
        self.changed.emit(self.index)

    def _on_edit(self) -> None:
        if not self._blocking:
            self.changed.emit(self.index)

    def set_band(self, band: FilterBand) -> None:
        b = band.clamped()
        self._blocking = True
        idx = self.type_combo.findData(b.type)
        if idx >= 0:
            self.type_combo.setCurrentIndex(idx)
        self.freq_spin.setValue(int(round(b.frequency)))
        self.gain_spin.setValue(b.gain)
        self.gain_slider.setValue(round(b.gain * 10))
        self.q_spin.setValue(b.q)
        self._blocking = False

    def band(self) -> FilterBand:
        return FilterBand(
            type=self.type_combo.currentData(),
            frequency=float(self.freq_spin.value()),
            gain=float(self.gain_spin.value()),
            q=float(self.q_spin.value()),
        ).clamped()


class MoondropDesktopWidget(QtWidgets.QWidget):
    """Floating KDE desktop widget for Moondrop USB DSP devices + AutoEQ."""

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Moondrop DSP Studio")
        self.setWindowIcon(QtGui.QIcon(str(ICON_PATH)))
        self.setWindowFlags(QtCore.Qt.WindowType.Window | QtCore.Qt.WindowType.FramelessWindowHint)
        self.setAttribute(QtCore.Qt.WidgetAttribute.WA_TranslucentBackground, True)
        self.resize(710, 800)

        self._presets = load_user_presets()
        self._entries: List[db.AutoEqEntry] = []
        self._moondrop_entries: List[db.AutoEqEntry] = []
        self._filtered_entries: List[db.AutoEqEntry] = []
        self._active_device: Optional[chu2_hid.MoondropDeviceInfo] = None
        self._active_device_key: Optional[str] = None
        self._active_band_count: int = 5
        self._active_tasks: List[_RunnableTask] = []
        self._shutting_down = False
        self._is_pinned = False
        self._hid_busy = False

        self._preview_timer = QtCore.QTimer(self)
        self._preview_timer.setSingleShot(True)
        self._preview_timer.setInterval(160)
        self._preview_timer.timeout.connect(lambda: self._push_to_device(commit=False))

        self._poll_timer = QtCore.QTimer(self)
        self._poll_timer.setInterval(2200)
        self._poll_timer.timeout.connect(self._poll_device_presence)

        self._build_ui()
        self._setup_tray()
        self._apply_stylesheet()

        self._poll_device_presence()
        self._poll_timer.start()
        self._load_autoeq_index(force_refresh=False)

    def _run_bg(
        self,
        fn: Callable[[], object],
        on_ok: Callable[[object], None],
        on_err: Callable[[str], None],
    ) -> None:
        if self._shutting_down:
            return
        task = _RunnableTask(fn)
        task.setAutoDelete(False)
        self._active_tasks.append(task)

        def _safe_ok(res: object) -> None:
            if task in self._active_tasks:
                self._active_tasks.remove(task)
            if not self._shutting_down:
                on_ok(res)

        def _safe_err(msg: str) -> None:
            if task in self._active_tasks:
                self._active_tasks.remove(task)
            if not self._shutting_down:
                on_err(msg)

        task.signals.finished.connect(_safe_ok, QtCore.Qt.ConnectionType.QueuedConnection)
        task.signals.failed.connect(_safe_err, QtCore.Qt.ConnectionType.QueuedConnection)
        QtCore.QThreadPool.globalInstance().start(task)

    def shutdown(self) -> None:
        self._shutting_down = True
        self._poll_timer.stop()
        self._preview_timer.stop()
        QtCore.QThreadPool.globalInstance().waitForDone(3000)
        self._active_tasks.clear()

    # ------------------------------------------------------------------ UI --
    def _build_ui(self) -> None:
        outer = QtWidgets.QVBoxLayout(self)
        outer.setContentsMargins(8, 8, 8, 8)

        self.shell = QtWidgets.QFrame()
        self.shell.setObjectName("Shell")
        outer.addWidget(self.shell)

        root = QtWidgets.QVBoxLayout(self.shell)
        root.setContentsMargins(14, 12, 14, 12)
        root.setSpacing(10)

        # Custom draggable header bar
        header = QtWidgets.QHBoxLayout()
        header.setSpacing(8)
        self._drag_bar = QtWidgets.QLabel("🎧  Moondrop DSP  •  Desktop Widget")
        self._drag_bar.setObjectName("TitleLabel")
        self._drag_bar.installEventFilter(self)
        header.addWidget(self._drag_bar, 1)

        self.status_pill = QtWidgets.QLabel("● Buscando Moondrop...")
        self.status_pill.setObjectName("StatusPill")
        header.addWidget(self.status_pill)

        self.pin_btn = QtWidgets.QToolButton()
        self.pin_btn.setText("📌 Fijar")
        self.pin_btn.setCheckable(True)
        self.pin_btn.setToolTip("Mantener widget siempre encima en el escritorio")
        self.pin_btn.toggled.connect(self._toggle_pin)
        header.addWidget(self.pin_btn)

        self.compact_btn = QtWidgets.QToolButton()
        self.compact_btn.setText("⊟ Mini")
        self.compact_btn.setCheckable(True)
        self.compact_btn.setToolTip("Alternar vista compacta / completa (AutoEQ)")
        self.compact_btn.toggled.connect(self._toggle_compact)
        header.addWidget(self.compact_btn)

        min_btn = QtWidgets.QToolButton()
        min_btn.setText("—")
        min_btn.setToolTip("Ocultar a la bandeja del sistema")
        min_btn.clicked.connect(self.hide)
        header.addWidget(min_btn)

        close_btn = QtWidgets.QToolButton()
        close_btn.setText("✕")
        close_btn.setToolTip("Ocultar a la bandeja (salir desde el icono del panel)")
        close_btn.clicked.connect(self.hide)
        header.addWidget(close_btn)
        root.addLayout(header)

        # Interactive Graph
        self.graph = PEQGraphWidget()
        self.graph.bandDragged.connect(self._on_graph_drag)
        self.graph.bandQScrolled.connect(self._on_graph_q_scroll)
        root.addWidget(self.graph)

        # PEQ bands (5 or 8 dynamically visible)
        bands_box = QtWidgets.QFrame()
        bands_box.setObjectName("Card")
        bands_layout = QtWidgets.QVBoxLayout(bands_box)
        bands_layout.setContentsMargins(6, 6, 6, 6)
        bands_layout.setSpacing(2)
        self.band_rows: List[BandRowWidget] = []
        for i in range(MAX_SUPPORTED_BANDS):
            row = BandRowWidget(i)
            row.changed.connect(self._on_band_row_changed)
            row.setVisible(i < self._active_band_count)
            self.band_rows.append(row)
            bands_layout.addWidget(row)
        root.addWidget(bands_box)

        # Hardware Action Bar
        hw_bar = QtWidgets.QHBoxLayout()
        hw_bar.setSpacing(8)

        self.live_chk = QtWidgets.QCheckBox("Preview en vivo")
        self.live_chk.setToolTip("Envía cambios a la RAM del DSP al mover controles (sin reiniciar USB)")
        self.live_chk.setChecked(True)
        hw_bar.addWidget(self.live_chk)

        flat_btn = QtWidgets.QPushButton("Poner Plano (0 dB)")
        flat_btn.setToolTip("Deja todas las bandas en 0.0 dB (sonido original sin EQ)")
        flat_btn.clicked.connect(self._apply_flat_now)
        hw_bar.addWidget(flat_btn)

        hw_bar.addStretch(1)

        read_btn = QtWidgets.QPushButton("Leer cable")
        read_btn.clicked.connect(self._read_from_device)
        hw_bar.addWidget(read_btn)

        preview_btn = QtWidgets.QPushButton("Preview RAM")
        preview_btn.clicked.connect(lambda: self._push_to_device(commit=False))
        hw_bar.addWidget(preview_btn)

        self.flash_btn = QtWidgets.QPushButton("Guardar en cable (Flash)")
        self.flash_btn.setObjectName("PrimaryBtn")
        self.flash_btn.clicked.connect(lambda: self._push_to_device(commit=True))
        hw_bar.addWidget(self.flash_btn)
        root.addLayout(hw_bar)

        # Collapsible AutoEQ + Presets Drawer
        self.drawer = QtWidgets.QWidget()
        drawer_layout = QtWidgets.QVBoxLayout(self.drawer)
        drawer_layout.setContentsMargins(0, 2, 0, 0)
        drawer_layout.setSpacing(8)

        preset_card = QtWidgets.QFrame()
        preset_card.setObjectName("Card")
        preset_row = QtWidgets.QHBoxLayout(preset_card)
        preset_row.setContentsMargins(8, 6, 8, 6)
        preset_row.setSpacing(6)
        preset_row.addWidget(QtWidgets.QLabel("Presets:"))

        self.preset_combo = QtWidgets.QComboBox()
        self.preset_combo.setMinimumWidth(180)
        preset_row.addWidget(self.preset_combo, 1)

        load_p_btn = QtWidgets.QPushButton("Cargar")
        load_p_btn.clicked.connect(self._load_selected_preset)
        preset_row.addWidget(load_p_btn)

        save_p_btn = QtWidgets.QPushButton("Guardar como...")
        save_p_btn.clicked.connect(self._save_preset_dialog)
        preset_row.addWidget(save_p_btn)

        del_p_btn = QtWidgets.QPushButton("Borrar")
        del_p_btn.clicked.connect(self._delete_selected_preset)
        preset_row.addWidget(del_p_btn)

        imp_btn = QtWidgets.QPushButton("Importar .txt")
        imp_btn.setToolTip("Importar archivo ParametricEQ.txt (EqualizerAPO)")
        imp_btn.clicked.connect(self._import_peq_txt)
        preset_row.addWidget(imp_btn)

        exp_btn = QtWidgets.QPushButton("Exportar .txt")
        exp_btn.clicked.connect(self._export_peq_txt)
        preset_row.addWidget(exp_btn)
        drawer_layout.addWidget(preset_card)

        # AutoEQ Database Card
        aeq_card = QtWidgets.QFrame()
        aeq_card.setObjectName("Card")
        aeq_layout = QtWidgets.QVBoxLayout(aeq_card)
        aeq_layout.setContentsMargins(10, 8, 10, 8)
        aeq_layout.setSpacing(7)

        aeq_top = QtWidgets.QHBoxLayout()
        aeq_title = QtWidgets.QLabel("Base de datos AutoEq (Moondrop Auto-Detect)")
        aeq_title.setStyleSheet("font-weight:700; color:#e2e8f0;")
        aeq_top.addWidget(aeq_title)

        self.db_status_lbl = QtWidgets.QLabel("Cargando índice...")
        self.db_status_lbl.setStyleSheet("color:#94a3b8; font-size:11px;")
        aeq_top.addWidget(self.db_status_lbl, 1)

        refresh_db_btn = QtWidgets.QPushButton("Actualizar BD")
        refresh_db_btn.clicked.connect(lambda: self._load_autoeq_index(force_refresh=True))
        aeq_top.addWidget(refresh_db_btn)
        aeq_layout.addLayout(aeq_top)

        # Source Moondrop model selector (filtered exclusively to Moondrop)
        src_row = QtWidgets.QHBoxLayout()
        src_row.addWidget(QtWidgets.QLabel("Tu Moondrop:"))
        self.moondrop_source_combo = QtWidgets.QComboBox()
        src_row.addWidget(self.moondrop_source_combo, 1)

        src_row.addWidget(QtWidgets.QLabel("Límite agudos:"))
        self.max_freq_combo = QtWidgets.QComboBox()
        self.max_freq_combo.addItem("10 kHz (rec. 711)", 10000.0)
        self.max_freq_combo.addItem("12 kHz", 12000.0)
        self.max_freq_combo.addItem("16 kHz", 16000.0)
        self.max_freq_combo.addItem("20 kHz (todo)", 20000.0)
        src_row.addWidget(self.max_freq_combo)
        aeq_layout.addLayout(src_row)

        self.mode_tabs = QtWidgets.QTabWidget()

        # Tab 1: Match another headphone
        tab_match = QtWidgets.QWidget()
        m_lay = QtWidgets.QVBoxLayout(tab_match)
        m_lay.setContentsMargins(4, 6, 4, 4)
        m_lay.setSpacing(6)

        search_row = QtWidgets.QHBoxLayout()
        self.search_input = QtWidgets.QLineEdit()
        self.search_input.setPlaceholderText("Buscar audífono o IEM destino (ej. Blessing 3, Truthear Hexa, HD 600, Kadenz)...")
        self.search_input.textChanged.connect(self._filter_phone_list)
        search_row.addWidget(self.search_input, 1)

        self.moondrop_only_chk = QtWidgets.QCheckBox("Solo Moondrop")
        self.moondrop_only_chk.setChecked(False)
        self.moondrop_only_chk.toggled.connect(self._filter_phone_list)
        search_row.addWidget(self.moondrop_only_chk)

        self.in_ear_only_chk = QtWidgets.QCheckBox("Solo In-Ear")
        self.in_ear_only_chk.setChecked(True)
        self.in_ear_only_chk.toggled.connect(self._filter_phone_list)
        search_row.addWidget(self.in_ear_only_chk)
        m_lay.addLayout(search_row)

        self.phone_list = QtWidgets.QListWidget()
        self.phone_list.setMinimumHeight(125)
        self.phone_list.setMaximumHeight(165)
        self.phone_list.itemDoubleClicked.connect(lambda _: self._run_match_phone())
        m_lay.addWidget(self.phone_list)

        match_btn_row = QtWidgets.QHBoxLayout()
        self.aeq_info_lbl = QtWidgets.QLabel("Selecciona un audífono destino y calcula el PEQ óptimo.")
        self.aeq_info_lbl.setStyleSheet("color:#94a3b8; font-size:11px;")
        self.aeq_info_lbl.setWordWrap(True)
        match_btn_row.addWidget(self.aeq_info_lbl, 1)

        self.calc_match_btn = QtWidgets.QPushButton("Igualar a este audífono")
        self.calc_match_btn.setObjectName("AccentBtn")
        self.calc_match_btn.clicked.connect(self._run_match_phone)
        match_btn_row.addWidget(self.calc_match_btn)
        m_lay.addLayout(match_btn_row)
        self.mode_tabs.addTab(tab_match, "AutoEQ a otro audífono / IEM")

        # Tab 2: Curva Target (Harman, AutoEq, JM-1...)
        tab_target = QtWidgets.QWidget()
        t_lay = QtWidgets.QVBoxLayout(tab_target)
        t_lay.setContentsMargins(4, 10, 4, 4)
        t_lay.setSpacing(8)
        t_row = QtWidgets.QHBoxLayout()
        t_row.addWidget(QtWidgets.QLabel("Curva objetivo:"))
        self.target_combo = QtWidgets.QComboBox()
        for label, fname in db.CURATED_TARGETS.items():
            self.target_combo.addItem(label, fname)
        t_row.addWidget(self.target_combo, 1)

        self.calc_target_btn = QtWidgets.QPushButton("Calcular AutoEQ al Target")
        self.calc_target_btn.setObjectName("AccentBtn")
        self.calc_target_btn.clicked.connect(self._run_target_curve)
        t_row.addWidget(self.calc_target_btn)
        t_lay.addLayout(t_row)

        t_desc = QtWidgets.QLabel(
            "Calcula los filtros PEQ para llevar tu modelo Moondrop hacia la curva seleccionada "
            "(Harman In-Ear 2019, AutoEq In-Ear, JM-1, Diffuse Field, etc.)."
        )
        t_desc.setWordWrap(True)
        t_desc.setStyleSheet("color:#94a3b8; font-size:11px;")
        t_lay.addWidget(t_desc)
        t_lay.addStretch(1)
        self.mode_tabs.addTab(tab_target, "AutoEQ a Curva Target")

        aeq_layout.addWidget(self.mode_tabs)
        drawer_layout.addWidget(aeq_card)
        root.addWidget(self.drawer)

        bottom = QtWidgets.QHBoxLayout()
        self.msg_lbl = QtWidgets.QLabel("Listo.")
        self.msg_lbl.setStyleSheet("color:#94a3b8; font-size:11px;")
        bottom.addWidget(self.msg_lbl, 1)
        grip = QtWidgets.QSizeGrip(self)
        bottom.addWidget(grip, 0, QtCore.Qt.AlignmentFlag.AlignBottom | QtCore.Qt.AlignmentFlag.AlignRight)
        root.addLayout(bottom)

        self._refresh_preset_combo()
        self._set_all_bands(FLAT_BANDS[:self._active_band_count], trigger_preview=False)

    def _setup_tray(self) -> None:
        self.tray = QtWidgets.QSystemTrayIcon(QtGui.QIcon(str(ICON_PATH)), self)
        self.tray.setToolTip("Moondrop DSP Studio")
        menu = QtWidgets.QMenu()
        act_toggle = menu.addAction("Mostrar / Ocultar widget")
        act_toggle.triggered.connect(self._toggle_visibility)
        menu.addSeparator()
        act_flat = menu.addAction("Poner Plano (0 dB)")
        act_flat.triggered.connect(self._apply_flat_now)
        act_factory = menu.addAction("Preset Fábrica Chu 2 DSP")
        act_factory.triggered.connect(lambda: self._set_all_bands(FACTORY_BANDS, trigger_preview=True))
        act_read = menu.addAction("Leer del cable")
        act_read.triggered.connect(self._read_from_device)
        menu.addSeparator()
        act_quit = menu.addAction("Salir")
        act_quit.triggered.connect(QtWidgets.QApplication.quit)
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(self._on_tray_activated)
        self.tray.show()

    def _apply_stylesheet(self) -> None:
        self.setStyleSheet(
            """
            QWidget { color: #e2e8f0; font-family: "Noto Sans", "Inter", sans-serif; font-size: 12px; }
            #Shell {
                background-color: rgba(18, 22, 33, 242);
                border: 1px solid #334155;
                border-radius: 14px;
            }
            #Card {
                background-color: #181e2c;
                border: 1px solid #283248;
                border-radius: 10px;
            }
            #TitleLabel { font-size: 14px; font-weight: 700; color: #f8fafc; padding: 2px 4px; }
            #StatusPill {
                background-color: #1e293b; border: 1px solid #334155;
                border-radius: 10px; padding: 3px 10px; font-size: 11px; font-weight: 600;
            }
            QPushButton, QToolButton {
                background-color: #232b3e; border: 1px solid #36415c;
                border-radius: 7px; padding: 5px 10px; color: #e2e8f0;
            }
            QPushButton:hover, QToolButton:hover { background-color: #2d3852; border-color: #4b5b80; }
            QToolButton:checked { background-color: #0284c7; border-color: #38bdf8; color: #ffffff; }
            #PrimaryBtn { background-color: #0284c7; border-color: #38bdf8; font-weight: 700; color: #ffffff; }
            #PrimaryBtn:hover { background-color: #0369a1; }
            #AccentBtn { background-color: #4f46e5; border-color: #818cf8; font-weight: 600; color: #ffffff; }
            #AccentBtn:hover { background-color: #4338ca; }
            QLineEdit, QComboBox, QSpinBox, QDoubleSpinBox, QListWidget {
                background-color: #10141f; border: 1px solid #2d374e;
                border-radius: 6px; padding: 4px 6px; color: #f1f5f9;
            }
            QListWidget::item:selected { background-color: #0284c7; color: #ffffff; border-radius: 4px; }
            QTabWidget::pane { border: 1px solid #283248; border-radius: 8px; background: #141926; }
            QTabBar::tab {
                background: #1b2233; padding: 5px 12px; border-top-left-radius: 6px; border-top-right-radius: 6px;
                margin-right: 3px; color: #94a3b8;
            }
            QTabBar::tab:selected { background: #26314a; color: #f8fafc; font-weight: 600; }
            QSlider::groove:horizontal { height: 5px; background: #26314a; border-radius: 2px; }
            QSlider::handle:horizontal {
                background: #38bdf8; width: 13px; height: 13px; margin: -4px 0; border-radius: 6px;
            }
            """
        )

    # ------------------------------------------------- Window / Tray events --
    def eventFilter(self, obj: QtCore.QObject, event: QtCore.QEvent) -> bool:
        if obj is self._drag_bar and event.type() == QtCore.QEvent.Type.MouseButtonPress:
            me = event
            if isinstance(me, QtGui.QMouseEvent) and me.button() == QtCore.Qt.MouseButton.LeftButton:
                win = self.windowHandle()
                if win is not None and win.startSystemMove():
                    return True
                self._drag_offset = me.globalPosition().toPoint() - self.frameGeometry().topLeft()
                return True
        if obj is self._drag_bar and event.type() == QtCore.QEvent.Type.MouseMove:
            me = event
            if isinstance(me, QtGui.QMouseEvent) and (me.buttons() & QtCore.Qt.MouseButton.LeftButton) and hasattr(self, "_drag_offset"):
                self.move(me.globalPosition().toPoint() - self._drag_offset)
                return True
        return super().eventFilter(obj, event)

    def _on_tray_activated(self, reason: QtWidgets.QSystemTrayIcon.ActivationReason) -> None:
        if reason in (QtWidgets.QSystemTrayIcon.ActivationReason.Trigger, QtWidgets.QSystemTrayIcon.ActivationReason.DoubleClick):
            self._toggle_visibility()

    def _toggle_visibility(self) -> None:
        if self.isVisible():
            self.hide()
        else:
            self.show()
            self.raise_()
            self.activateWindow()

    def _toggle_pin(self, pinned: bool) -> None:
        self._is_pinned = pinned
        self.setWindowFlag(QtCore.Qt.WindowType.WindowStaysOnTopHint, pinned)
        self.show()

    def _toggle_compact(self, compact: bool) -> None:
        self.drawer.setVisible(not compact)
        self.adjustSize()

    # ------------------------------------------------------------ EQ Logic --
    def _configure_bands_ui(self, count: int, max_gain_db: float, supports_shelves: bool) -> None:
        self._active_band_count = count
        for i, row in enumerate(self.band_rows):
            row.setVisible(i < count)
            row.set_constraints(max_gain_db=max_gain_db, supports_shelves=supports_shelves)

    def _current_bands(self) -> List[FilterBand]:
        return [r.band() for r in self.band_rows[:self._active_band_count]]

    def _set_all_bands(self, bands: Sequence[FilterBand], trigger_preview: bool = False) -> None:
        for i in range(self._active_band_count):
            b = bands[i] if i < len(bands) else FLAT_BANDS[i]
            self.band_rows[i].set_band(b)
        self.graph.set_bands(self._current_bands())
        if trigger_preview and self.live_chk.isChecked():
            self._preview_timer.start()

    def _apply_flat_now(self) -> None:
        self.graph.set_target_overlay(None, None)
        self._set_all_bands(FLAT_BANDS[:self._active_band_count], trigger_preview=True)
        self._set_msg("EQ puesto en plano (0.0 dB).")

    def _on_band_row_changed(self, idx: int) -> None:
        self.graph.set_selected(idx)
        self.graph.set_bands(self._current_bands())
        if self.live_chk.isChecked():
            self._preview_timer.start()

    def _on_graph_drag(self, idx: int, freq: float, gain: float) -> None:
        if idx >= self._active_band_count:
            return
        row = self.band_rows[idx]
        b = row.band()
        row.set_band(FilterBand(b.type, freq, gain, b.q))
        self.graph.set_bands(self._current_bands())
        if self.live_chk.isChecked():
            self._preview_timer.start()

    def _on_graph_q_scroll(self, idx: int, new_q: float) -> None:
        if idx >= self._active_band_count:
            return
        row = self.band_rows[idx]
        b = row.band()
        row.set_band(FilterBand(b.type, b.frequency, b.gain, new_q))
        self.graph.set_bands(self._current_bands())
        if self.live_chk.isChecked():
            self._preview_timer.start()

    # --------------------------------------------------------- HID Device --
    def _sync_moondrop_source_selection(self) -> None:
        if not self._moondrop_entries or self._active_device is None:
            return
        idx = db.find_best_moondrop_index(self._moondrop_entries, self._active_device.autoeq_hint)
        if idx >= 0:
            self.moondrop_source_combo.setCurrentIndex(idx)

    def _poll_device_presence(self) -> None:
        if self._hid_busy or self._shutting_down:
            return
        dev = chu2_hid.find_active_moondrop_device()
        self._active_device = dev
        if dev:
            key = f"{dev.hidraw_path}:{dev.vid_pid}:{dev.hid_name}"
            self._drag_bar.setText(f"🎧  {dev.display_name}  •  Desktop Widget")
            self.status_pill.setText(f"● {dev.display_name} ({os.path.basename(dev.hidraw_path)})")
            self.status_pill.setStyleSheet(
                "background:#062e24; border:1px solid #10b981; color:#6ee7b7; border-radius:10px; padding:3px 10px; font-size:11px; font-weight:600;"
            )
            if key != self._active_device_key:
                self._active_device_key = key
                self._configure_bands_ui(dev.max_bands, dev.max_gain_db, dev.supports_shelves)
                self._sync_moondrop_source_selection()
                self._read_from_device()
        else:
            self._active_device_key = None
            self._drag_bar.setText("🎧  Moondrop DSP  •  Desktop Widget")
            self.status_pill.setText("○ Sin Moondrop DSP conectado")
            self.status_pill.setStyleSheet(
                "background:#2a171c; border:1px solid #ef4444; color:#fca5a5; border-radius:10px; padding:3px 10px; font-size:11px; font-weight:600;"
            )

    def _read_from_device(self) -> None:
        if self._hid_busy:
            return
        dev = chu2_hid.find_active_moondrop_device()
        if not dev:
            self._set_msg("No hay ningún dispositivo Moondrop conectado.", is_err=True)
            return
        self._hid_busy = True
        self._set_msg(f"Leyendo ecualización de {dev.display_name}...")

        def task() -> Tuple[chu2_hid.MoondropDeviceInfo, bool, List[FilterBand]]:
            return chu2_hid.read_chu2_eq(dev)

        def ok(res: object) -> None:
            self._hid_busy = False
            info, enabled, bands = res  # type: ignore[misc]
            self._configure_bands_ui(info.max_bands, info.max_gain_db, info.supports_shelves)
            self.graph.set_target_overlay(None, None)
            self._set_all_bands(bands, trigger_preview=False)
            self._set_msg(
                f"Leído {info.display_name} ({os.path.basename(info.hidraw_path)}, "
                f"DSP {'activo' if enabled else 'apagado'})."
            )

        def err(msg: str) -> None:
            self._hid_busy = False
            self._set_msg(f"Error leyendo DSP: {msg}", is_err=True)

        self._run_bg(task, ok, err)

    def _push_to_device(self, commit: bool) -> None:
        if self._hid_busy:
            return
        dev = chu2_hid.find_active_moondrop_device()
        if not dev:
            self._set_msg("Conecta un dispositivo Moondrop DSP primero.", is_err=True)
            return
        bands = self._current_bands()
        self._hid_busy = True
        self.flash_btn.setEnabled(False)
        self._set_msg(
            f"Guardando en flash de {dev.display_name}..." if commit else f"Aplicando preview en {dev.display_name}..."
        )

        def task() -> chu2_hid.MoondropDeviceInfo:
            return chu2_hid.write_chu2_eq(bands, commit=commit, enable_eq=True, info=dev)

        def ok(info_obj: object) -> None:
            self._hid_busy = False
            self.flash_btn.setEnabled(True)
            info = info_obj  # type: ignore[assignment]
            if commit:
                self._set_msg(f"Guardado permanentemente en {info.display_name}.")
            else:
                self._set_msg(f"Preview aplicado en RAM ({info.display_name}).")

        def err(msg: str) -> None:
            self._hid_busy = False
            self.flash_btn.setEnabled(True)
            self._set_msg(f"Error escribiendo DSP: {msg}", is_err=True)

        self._run_bg(task, ok, err)

    # ------------------------------------------------------------ Presets --
    def _refresh_preset_combo(self) -> None:
        cur = self.preset_combo.currentText()
        self.preset_combo.clear()
        for name in self._presets:
            self.preset_combo.addItem(name)
        idx = self.preset_combo.findText(cur)
        if idx >= 0:
            self.preset_combo.setCurrentIndex(idx)

    def _load_selected_preset(self) -> None:
        name = self.preset_combo.currentText()
        if name in self._presets:
            self.graph.set_target_overlay(None, None)
            self._set_all_bands(self._presets[name], trigger_preview=True)
            self._set_msg(f"Preset '{name}' cargado.")

    def _save_preset_dialog(self) -> None:
        name, ok = QtWidgets.QInputDialog.getText(self, "Guardar Preset", "Nombre del preset:")
        if ok and name.strip():
            clean = name.strip()
            self._presets[clean] = self._current_bands()
            save_user_presets(self._presets)
            self._refresh_preset_combo()
            self.preset_combo.setCurrentText(clean)
            self._set_msg(f"Preset '{clean}' guardado.")

    def _delete_selected_preset(self) -> None:
        name = self.preset_combo.currentText()
        if name in _default_presets():
            self._set_msg("Los presets base no se eliminan.", is_err=True)
            return
        if name in self._presets:
            del self._presets[name]
            save_user_presets(self._presets)
            self._refresh_preset_combo()
            self._set_msg(f"Preset '{name}' eliminado.")

    def _import_peq_txt(self) -> None:
        path, _ = QtWidgets.QFileDialog.getOpenFileName(self, "Importar ParametricEQ.txt", str(Path.home()), "Text (*.txt);;All (*)")
        if not path:
            return
        try:
            bands = db.parse_parametric_eq(Path(path).read_text(encoding="utf-8"))
            if not bands:
                raise ValueError("No se encontraron filtros PK/LSC/HSC válidos.")
            self.graph.set_target_overlay(None, None)
            self._set_all_bands(bands[:self._active_band_count], trigger_preview=True)
            self._set_msg(f"Importadas {min(len(bands), self._active_band_count)} bandas desde {os.path.basename(path)}.")
        except Exception as exc:
            self._set_msg(f"Error al importar: {exc}", is_err=True)

    def _export_peq_txt(self) -> None:
        path, _ = QtWidgets.QFileDialog.getSaveFileName(self, "Exportar ParametricEQ.txt", str(Path.home() / "moondrop_dsp_peq.txt"), "Text (*.txt)")
        if not path:
            return
        Path(path).write_text(db.export_parametric_eq(self._current_bands()), encoding="utf-8")
        self._set_msg(f"Exportado a {os.path.basename(path)}.")

    # ---------------------------------------------------- AutoEQ Database --
    def _load_autoeq_index(self, force_refresh: bool) -> None:
        self.db_status_lbl.setText("Descargando índice AutoEq...")

        def task() -> List[db.AutoEqEntry]:
            return db.load_index(force_refresh=force_refresh)

        def ok(entries: object) -> None:
            self._entries = entries  # type: ignore[assignment]
            self._moondrop_entries = db.find_moondrop_entries(self._entries)
            self.moondrop_source_combo.clear()
            for c in self._moondrop_entries:
                self.moondrop_source_combo.addItem(c.label, c)
            self._sync_moondrop_source_selection()
            self.db_status_lbl.setText(
                f"{len(self._moondrop_entries)} mediciones Moondrop  |  {len(self._entries):,} totales"
            )
            self._filter_phone_list()

        def err(msg: str) -> None:
            self.db_status_lbl.setText("Error cargando BD")
            self._set_msg(f"Error de red con AutoEq: {msg}", is_err=True)

        self._run_bg(task, ok, err)

    def _filter_phone_list(self) -> None:
        query = self.search_input.text().strip().lower()
        tokens = query.split()
        in_ear_only = self.in_ear_only_chk.isChecked()
        moondrop_only = self.moondrop_only_chk.isChecked()

        matches: List[db.AutoEqEntry] = []
        for e in self._entries:
            if in_ear_only and e.form_factor != "in-ear":
                continue
            if moondrop_only and "moondrop" not in e.name.lower():
                continue
            hay = f"{e.name} {e.source_rig}".lower()
            if all(tok in hay for tok in tokens):
                matches.append(e)

        def _rank(e: db.AutoEqEntry) -> Tuple[int, str]:
            s = e.source_rig.lower()
            if "crinacle on 711" in s:
                return (0, e.name.lower())
            if "super review" in s:
                return (1, e.name.lower())
            if "oratory1990" in s:
                return (2, e.name.lower())
            if e.form_factor == "in-ear":
                return (3, e.name.lower())
            return (4, e.name.lower())

        matches.sort(key=_rank)
        matches = matches[:180]
        self._filtered_entries = matches
        self.phone_list.clear()
        for e in matches:
            self.phone_list.addItem(f"{e.name}   [{e.source_rig}]")
        if matches:
            self.phone_list.setCurrentRow(0)

    def _device_constraints(self) -> Tuple[int, float, bool]:
        if self._active_device is not None:
            return (
                self._active_device.max_bands,
                self._active_device.max_gain_db,
                self._active_device.supports_shelves,
            )
        return (5, 12.0, True)

    def _run_match_phone(self) -> None:
        if not self._moondrop_entries:
            self._set_msg("Aún no se cargó la base de datos AutoEq.", is_err=True)
            return
        row_idx = self.phone_list.currentRow()
        if row_idx < 0 or row_idx >= len(self._filtered_entries):
            self._set_msg("Selecciona un audífono de la lista.", is_err=True)
            return

        src_moondrop: db.AutoEqEntry = self.moondrop_source_combo.currentData()
        tgt_phone = self._filtered_entries[row_idx]
        max_f = float(self.max_freq_combo.currentData())
        max_b, max_g, shelves = self._device_constraints()

        self.calc_match_btn.setEnabled(False)
        self.aeq_info_lbl.setText(f"Optimizando {max_b} bandas: {src_moondrop.name} → {tgt_phone.name}...")

        def task() -> Tuple[np.ndarray, np.ndarray, List[FilterBand], float, Optional[str]]:
            return db.compute_match_headphone_peq(
                src_moondrop,
                tgt_phone,
                max_freq_fit=max_f,
                max_bands=max_b,
                max_gain_db=max_g,
                supports_shelves=shelves,
            )

        def ok(res: object) -> None:
            self.calc_match_btn.setEnabled(True)
            freqs, delta_db, bands, rmse, warn = res  # type: ignore[misc]
            self.graph.set_target_overlay(freqs, delta_db)
            self._set_all_bands(bands, trigger_preview=True)
            info = f"{src_moondrop.name} → {tgt_phone.name} ({tgt_phone.source_rig}) • RMSE: {rmse:.2f} dB"
            if warn:
                info += f" — {warn}"
            self.aeq_info_lbl.setText(info)
            self._set_msg(f"AutoEQ calculado ({tgt_phone.name}, RMSE {rmse:.2f} dB).")

        def err(msg: str) -> None:
            self.calc_match_btn.setEnabled(True)
            self.aeq_info_lbl.setText(f"Error: {msg}")
            self._set_msg(f"Error calculando AutoEQ: {msg}", is_err=True)

        self._run_bg(task, ok, err)

    def _run_target_curve(self) -> None:
        if not self._moondrop_entries:
            self._set_msg("Aún no se cargó la base de datos AutoEq.", is_err=True)
            return
        src_moondrop: db.AutoEqEntry = self.moondrop_source_combo.currentData()
        target_csv = self.target_combo.currentData()
        target_label = self.target_combo.currentText()
        max_f = float(self.max_freq_combo.currentData())
        max_b, max_g, shelves = self._device_constraints()

        self.calc_target_btn.setEnabled(False)
        self._set_msg(f"Calculando AutoEQ ({src_moondrop.name}) hacia {target_label}...")

        def task() -> Tuple[np.ndarray, np.ndarray, List[FilterBand], float]:
            return db.compute_target_curve_peq(
                src_moondrop,
                target_csv,
                max_freq_fit=max_f,
                max_bands=max_b,
                max_gain_db=max_g,
                supports_shelves=shelves,
            )

        def ok(res: object) -> None:
            self.calc_target_btn.setEnabled(True)
            freqs, target_db, bands, rmse = res  # type: ignore[misc]
            self.graph.set_target_overlay(freqs, target_db)
            self._set_all_bands(bands, trigger_preview=True)
            self.aeq_info_lbl.setText(f"{src_moondrop.name} → '{target_label}' • RMSE: {rmse:.2f} dB")
            self._set_msg(f"AutoEQ '{target_label}' aplicado (RMSE {rmse:.2f} dB).")

        def err(msg: str) -> None:
            self.calc_target_btn.setEnabled(True)
            self._set_msg(f"Error calculando target: {msg}", is_err=True)

        self._run_bg(task, ok, err)

    def _set_msg(self, text: str, is_err: bool = False) -> None:
        self.msg_lbl.setText(text)
        self.msg_lbl.setStyleSheet(
            "color:#fca5a5; font-size:11px;" if is_err else "color:#94a3b8; font-size:11px;"
        )


def main() -> int:
    app = QtWidgets.QApplication(sys.argv)
    app.setQuitOnLastWindowClosed(False)
    app.setApplicationName("Moondrop DSP Studio")
    win = MoondropDesktopWidget()
    app.aboutToQuit.connect(win.shutdown)
    win.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
