"""Windows USB HID transport (via hidapi) for Moondrop USB-C DSP devices [BETA — UNTESTED].

Implements the same binary protocol as `chu2_hid.py` (Linux) and `MoondropProtocolCodec.kt` (Android):
  - KTMicro 5-band devices (VID 0x31B2): Moondrop Chu 2 DSP (0x0113), CDSP, Quark 2
  - Moondrop 8-band devices: FreeDSP Pro/Mini, Rays, AG Rays, Marigold, DAWN PRO 2, Echo A, MOONRIVER 3, DHA15
"""

from __future__ import annotations

import dataclasses
import struct
from typing import List, Optional, Sequence, Tuple

from biquad import FilterBand, FilterType

try:
    import hid  # type: ignore
except ImportError:
    hid = None  # type: ignore

KT_VID = 0x31B2
CHU2_DSP_PID = 0x0113
KT_REPORT_ID = 0x4B
KT_READ = 0x52
KT_WRITE = 0x57
KT_COMMIT = 0x53
KT_REG_SLOT = 0x24
KT_REG_BAND0 = 0x26
KT_SLOT_ON = 0x03
KT_SLOT_OFF = 0x02

_KT_TYPE_TO_CODE = {"peaking": 0, "low_shelf": 3, "high_shelf": 4}
_KT_CODE_TO_TYPE: dict[int, FilterType] = {0: "peaking", 3: "low_shelf", 4: "high_shelf"}

_MOONDROP8_SPECS: List[Tuple[str, str, float, bool, str]] = [
    ("ag rays", "Moondrop AG Rays", 12.0, False, "Moondrop"),
    ("rays", "Moondrop Rays", 12.0, False, "Moondrop"),
    ("marigold", "Moondrop Marigold", 6.0, False, "Moondrop"),
    ("freedsp pro", "Moondrop FreeDSP Pro", 12.0, True, "Moondrop"),
    ("freedsp mini", "Moondrop FreeDSP Mini", 12.0, True, "Moondrop"),
    ("dawn pro2", "Moondrop DAWN PRO 2", 12.0, True, "Moondrop"),
    ("echo a", "Moondrop Echo A", 12.0, False, "Moondrop"),
    ("moonriver 3", "Moondrop MOONRIVER 3", 12.0, True, "Moondrop"),
    ("dha15", "Moondrop DHA15", 12.0, True, "Moondrop"),
]


class WinHidError(RuntimeError):
    """Raised when a Windows HID operation fails."""


@dataclasses.dataclass(frozen=True)
class MoondropDeviceInfo:
    hid_path: bytes
    vid: int
    pid: int
    hid_name: str
    display_name: str
    protocol: str  # "ktmicro" | "moondrop8"
    max_bands: int
    max_gain_db: float
    supports_shelves: bool
    disconnects_on_commit: bool
    autoeq_hint: str

    @property
    def vid_pid(self) -> str:
        return f"{self.vid:04x}:{self.pid:04x}"

    @property
    def hidraw_path(self) -> str:
        return self.hid_path.decode("utf-8", errors="ignore")


def detect_devices() -> List[MoondropDeviceInfo]:
    """Enumerate connected Moondrop USB-C DSP HID interfaces on Windows."""
    if hid is None:
        return []
    found: List[MoondropDeviceInfo] = []
    seen: set[bytes] = set()
    try:
        entries = hid.enumerate()
    except Exception:
        return []

    for item in entries:
        vid = int(item.get("vendor_id", 0))
        pid = int(item.get("product_id", 0))
        path = item.get("path", b"")
        if not path or path in seen:
            continue
        prod = str(item.get("product_string") or "")
        low = prod.lower()

        if vid == KT_VID:
            seen.add(path)
            if pid == CHU2_DSP_PID or "chu2" in low or "chu 2" in low:
                name, hint = "Moondrop Chu 2 DSP", "Moondrop Chu 2"
            else:
                name, hint = prod or f"Moondrop KTMicro ({vid:04x}:{pid:04x})", "Moondrop"
            found.append(
                MoondropDeviceInfo(
                    hid_path=path,
                    vid=vid,
                    pid=pid,
                    hid_name=prod,
                    display_name=name,
                    protocol="ktmicro",
                    max_bands=5,
                    max_gain_db=12.0,
                    supports_shelves=True,
                    disconnects_on_commit=True,
                    autoeq_hint=hint,
                )
            )
            continue

        for substr, disp, max_g, shelves, hint in _MOONDROP8_SPECS:
            if substr in low:
                seen.add(path)
                found.append(
                    MoondropDeviceInfo(
                        hid_path=path,
                        vid=vid,
                        pid=pid,
                        hid_name=prod,
                        display_name=disp,
                        protocol="moondrop8",
                        max_bands=8,
                        max_gain_db=max_g,
                        supports_shelves=shelves,
                        disconnects_on_commit=False,
                        autoeq_hint=hint,
                    )
                )
                break
    return found


def _kt_packet(reg: int, cmd: int, value: bytes = b"\x00\x00\x00\x00") -> bytes:
    pkt = bytearray(11)
    pkt[0] = KT_REPORT_ID
    pkt[1] = reg & 0xFF
    pkt[5] = cmd & 0xFF
    pkt[7 : 7 + min(4, len(value))] = value[:4]
    return bytes(pkt)


def _kt_encode_band(band: FilterBand) -> Tuple[bytes, bytes]:
    b = band.clamped()
    gf = struct.pack("<hH", int(round(b.gain * 10.0)), int(round(b.frequency)))
    qt = struct.pack("<HBB", int(round(b.q * 1000.0)), _KT_TYPE_TO_CODE.get(b.type, 0), 0)
    return gf, qt


def _kt_decode_band(gf: bytes, qt: bytes) -> FilterBand:
    gain_x10, freq = struct.unpack("<hH", gf[:4])
    q_x1000, tcode, _ = struct.unpack("<HBB", qt[:4])
    ftype: FilterType = _KT_CODE_TO_TYPE.get(tcode, "peaking")
    return FilterBand(type=ftype, frequency=float(freq), gain=gain_x10 / 10.0, q=q_x1000 / 1000.0).clamped()


class MoondropWinHidDevice:
    """Context manager for reading/writing EQ over Windows hidapi (KTMicro 5-band)."""

    def __init__(self, info: MoondropDeviceInfo) -> None:
        self.info = info
        self._dev = None

    def __enter__(self) -> "MoondropWinHidDevice":
        if hid is None:
            raise WinHidError("El paquete Python 'hidapi' no está instalado (pip install hidapi).")
        dev = hid.device()
        try:
            dev.open_path(self.info.hid_path)
            dev.set_nonblocking(False)
        except Exception as exc:
            raise WinHidError(f"No se pudo abrir el dispositivo HID en Windows: {exc}") from exc
        self._dev = dev
        return self

    def __exit__(self, *_: object) -> None:
        if self._dev is not None:
            try:
                self._dev.close()
            except Exception:
                pass
            self._dev = None

    def _kt_cmd(self, reg: int, cmd: int, value: bytes = b"\x00\x00\x00\x00", ignore_disconnect: bool = False) -> bytes:
        assert self._dev is not None
        pkt = _kt_packet(reg, cmd, value)
        try:
            self._dev.write(pkt)
            if ignore_disconnect:
                return b"\x00\x00\x00\x00"
            resp = bytes(self._dev.read(64, timeout_ms=400))
            if len(resp) < 11:
                raise WinHidError(f"Respuesta HID corta ({len(resp)} bytes) en registro 0x{reg:02x}")
            return resp[7:11]
        except Exception as exc:
            if ignore_disconnect:
                return b"\x00\x00\x00\x00"
            raise WinHidError(f"Error HID en registro 0x{reg:02x}: {exc}") from exc

    def read_eq(self) -> Tuple[bool, List[FilterBand]]:
        if self.info.protocol != "ktmicro":
            raise WinHidError("Protocolo Moondrop 8-band en Windows está pendiente de pruebas en hardware.")
        slot = self._kt_cmd(KT_REG_SLOT, KT_READ)[0]
        enabled = slot != KT_SLOT_OFF
        bands: List[FilterBand] = []
        for i in range(5):
            reg = KT_REG_BAND0 + 2 * i
            gf = self._kt_cmd(reg, KT_READ)
            qt = self._kt_cmd(reg + 1, KT_READ)
            bands.append(_kt_decode_band(gf, qt))
        return enabled, bands

    def write_eq(self, bands: Sequence[FilterBand], commit: bool = False, enable_eq: bool = True) -> None:
        if self.info.protocol != "ktmicro":
            raise WinHidError("Protocolo Moondrop 8-band en Windows está pendiente de pruebas en hardware.")
        fitted = [b.clamped() for b in list(bands)[:5]]
        while len(fitted) < 5:
            fitted.append(FilterBand("peaking", 1000.0, 0.0, 1.0))

        slot_val = bytes([KT_SLOT_ON if enable_eq else KT_SLOT_OFF, 0, 0, 0])
        self._kt_cmd(KT_REG_SLOT, KT_WRITE, slot_val)

        for i, band in enumerate(fitted):
            reg = KT_REG_BAND0 + 2 * i
            gf, qt = _kt_encode_band(band)
            self._kt_cmd(reg, KT_WRITE, gf)
            self._kt_cmd(reg + 1, KT_WRITE, qt)
            if self._kt_cmd(reg, KT_READ) != gf or self._kt_cmd(reg + 1, KT_READ) != qt:
                raise WinHidError(f"Verificación fallida al escribir la banda {i + 1}.")

        if commit:
            self._kt_cmd(0x00, KT_COMMIT, ignore_disconnect=True)
