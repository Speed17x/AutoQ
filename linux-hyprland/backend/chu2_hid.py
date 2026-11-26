"""Auto-detection and direct Linux /dev/hidraw transport for Moondrop USB DSP devices.

Supports:
1. KTMicro 5-band devices (VID 0x31B2): Moondrop Chu 2 DSP (0x0113), CDSP, Quark 2, etc.
2. Moondrop 8-band devices (moondropUsbHidHandler): Rays, AG Rays, Marigold,
   FreeDSP Pro, FreeDSP Mini, DAWN PRO2, Echo A, MOONRIVER 3, DHA15.
"""

from __future__ import annotations

import dataclasses
import glob
import math
import os
import select
import time
from typing import List, Optional, Sequence, Tuple

from biquad import FilterBand, FilterType

# KTMicro protocol constants (Chu2 DSP, CDSP, etc.)
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
_KT_CODE_TO_TYPE = {v: k for k, v in _KT_TYPE_TO_CODE.items()}

# Moondrop 8-band HID protocol constants (FreeDSP Pro/Mini, Rays, Marigold, Dawn Pro 2, etc.)
MD_REPORT_ID = 0x4B
MD_CMD_WRITE = 0x01
MD_CMD_READ = 0x80
MD_CMD_UPDATE_EQ = 0x09
MD_CMD_APPLY_COEFF = 0x0A
MD_CMD_SAVE_FLASH = 0x01
MD_CMD_ACTIVE_EQ = 0x0F
_MD_TYPE_TO_CODE = {"low_shelf": 1, "peaking": 2, "high_shelf": 3}
_MD_CODE_TO_TYPE: dict[int, FilterType] = {1: "low_shelf", 2: "peaking", 3: "high_shelf"}


class Chu2HidError(RuntimeError):
    """Raised when a Moondrop DSP device cannot be opened or a HID command fails."""


@dataclasses.dataclass(frozen=True)
class MoondropDeviceInfo:
    hidraw_path: str
    vid: int
    pid: int
    hid_name: str
    display_name: str
    protocol: str          # "ktmicro" | "moondrop8"
    max_bands: int
    max_gain_db: float
    supports_shelves: bool
    disconnects_on_commit: bool
    autoeq_hint: str       # Search hint for AutoEq Moondrop models

    @property
    def vid_pid(self) -> str:
        return f"{self.vid:04x}:{self.pid:04x}"


# Known Moondrop 8-band HID names (case-insensitive substring match)
_MOONDROP8_SPECS: List[Tuple[str, str, float, bool, str]] = [
    # (match_substr, display_name, max_gain, supports_shelves, autoeq_hint)
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


def _classify_hidraw(dev_path: str, vid: int, pid: int, hid_name: str) -> Optional[MoondropDeviceInfo]:
    low = hid_name.lower()

    # 1. KTMicro Moondrop devices (VID 0x31B2)
    if vid == KT_VID:
        if pid == CHU2_DSP_PID or "chu2" in low or "chu 2" in low:
            return MoondropDeviceInfo(
                hidraw_path=dev_path, vid=vid, pid=pid, hid_name=hid_name,
                display_name="Moondrop Chu 2 DSP", protocol="ktmicro",
                max_bands=5, max_gain_db=12.0, supports_shelves=True,
                disconnects_on_commit=True, autoeq_hint="Moondrop Chu 2",
            )
        if "quark" in low:
            return MoondropDeviceInfo(
                hidraw_path=dev_path, vid=vid, pid=pid, hid_name=hid_name,
                display_name="Moondrop Quark 2 DSP", protocol="ktmicro",
                max_bands=5, max_gain_db=12.0, supports_shelves=True,
                disconnects_on_commit=True, autoeq_hint="Moondrop",
            )
        if "jiu" in low:
            return MoondropDeviceInfo(
                hidraw_path=dev_path, vid=vid, pid=pid, hid_name=hid_name,
                display_name="Moondrop Jiu DSP", protocol="ktmicro",
                max_bands=5, max_gain_db=12.0, supports_shelves=True,
                disconnects_on_commit=True, autoeq_hint="Moondrop Jiu",
            )
        if "may" in low:
            return MoondropDeviceInfo(
                hidraw_path=dev_path, vid=vid, pid=pid, hid_name=hid_name,
                display_name="Moondrop May DSP", protocol="ktmicro",
                max_bands=5, max_gain_db=12.0, supports_shelves=True,
                disconnects_on_commit=True, autoeq_hint="Moondrop May",
            )
        if "cdsp" in low or "moondrop" in low:
            return MoondropDeviceInfo(
                hidraw_path=dev_path, vid=vid, pid=pid, hid_name=hid_name,
                display_name=f"Moondrop {hid_name or 'CDSP'}", protocol="ktmicro",
                max_bands=5, max_gain_db=12.0, supports_shelves=True,
                disconnects_on_commit=True, autoeq_hint="Moondrop",
            )

    # 2. Moondrop 8-band HID devices
    for substr, disp, max_g, shelves, hint in _MOONDROP8_SPECS:
        if substr in low:
            return MoondropDeviceInfo(
                hidraw_path=dev_path, vid=vid, pid=pid, hid_name=hid_name,
                display_name=disp, protocol="moondrop8",
                max_bands=8, max_gain_db=max_g, supports_shelves=shelves,
                disconnects_on_commit=False, autoeq_hint=hint,
            )

    return None


def scan_moondrop_devices() -> List[MoondropDeviceInfo]:
    """Return all connected Moondrop USB DSP devices visible via /sys/class/hidraw."""
    found: List[MoondropDeviceInfo] = []
    for uevent_path in sorted(glob.glob("/sys/class/hidraw/hidraw*/device/uevent")):
        try:
            with open(uevent_path, "r", encoding="utf-8", errors="ignore") as f:
                content = f.read()
        except OSError:
            continue
        vid = 0
        pid = 0
        hid_name = ""
        for line in content.splitlines():
            if line.startswith("HID_ID="):
                parts = line.split("=", 1)[1].strip().split(":")
                if len(parts) == 3:
                    try:
                        vid = int(parts[1], 16)
                        pid = int(parts[2], 16)
                    except ValueError:
                        pass
            elif line.startswith("HID_NAME="):
                hid_name = line.split("=", 1)[1].strip()
        hidraw_name = uevent_path.split("/")[4]
        dev_path = f"/dev/{hidraw_name}"
        if not os.path.exists(dev_path):
            continue
        info = _classify_hidraw(dev_path, vid, pid, hid_name)
        if info is not None:
            found.append(info)
    return found


def find_chu2_hidraw() -> Optional[str]:
    """Back-compat helper returning the first detected Moondrop DSP /dev/hidrawX path."""
    devs = scan_moondrop_devices()
    return devs[0].hidraw_path if devs else None


def find_active_moondrop_device() -> Optional[MoondropDeviceInfo]:
    devs = scan_moondrop_devices()
    return devs[0] if devs else None


# --------------------------------------------------------------------------- #
# KTMicro protocol helpers (5-band)
# --------------------------------------------------------------------------- #
def _kt_packet(reg: int, cmd: int, value: bytes = bytes(4)) -> bytes:
    return bytes([KT_REPORT_ID, reg, 0, 0, 0, cmd, 0]) + value


def _kt_encode_band(band: FilterBand) -> Tuple[bytes, bytes]:
    b = band.clamped()
    gain_freq = (
        round(b.gain * 10).to_bytes(2, "little", signed=True)
        + round(b.frequency).to_bytes(2, "little")
    )
    q_type = round(b.q * 1000).to_bytes(2, "little") + bytes([_KT_TYPE_TO_CODE[b.type], 0])
    return gain_freq, q_type


def _kt_decode_band(gain_freq: bytes, q_type: bytes) -> FilterBand:
    ftype: FilterType = _KT_CODE_TO_TYPE.get(q_type[2], "peaking")  # type: ignore[assignment]
    gain = int.from_bytes(gain_freq[0:2], "little", signed=True) / 10.0
    freq = float(int.from_bytes(gain_freq[2:4], "little"))
    q = int.from_bytes(q_type[0:2], "little") / 1000.0
    return FilterBand(type=ftype, frequency=freq, gain=gain, q=q).clamped()


# --------------------------------------------------------------------------- #
# Moondrop 8-band protocol helpers (96 kHz biquad coeffs)
# --------------------------------------------------------------------------- #
def _md8_encode_biquad(band: FilterBand) -> List[int]:
    b = band.clamped()
    A = 10.0 ** (b.gain / 40.0)
    w0 = 2.0 * math.pi * b.frequency / 96000.0
    sin_w0 = math.sin(w0)
    cos_w0 = math.cos(w0)
    alpha = sin_w0 / (2.0 * b.q)

    if b.type in ("low_shelf", "high_shelf"):
        shelf_alpha = (sin_w0 / 2.0) * math.sqrt(max(1e-9, (A + 1.0 / A) * (1.0 / b.q - 1.0) + 2.0))
        s = 2.0 * math.sqrt(A) * shelf_alpha
        if b.type == "low_shelf":
            b0 = A * ((A + 1.0) - (A - 1.0) * cos_w0 + s)
            b1 = 2.0 * A * ((A - 1.0) - (A + 1.0) * cos_w0)
            b2 = A * ((A + 1.0) - (A - 1.0) * cos_w0 - s)
            a0 = (A + 1.0) + (A - 1.0) * cos_w0 + s
            a1 = -2.0 * ((A - 1.0) + (A + 1.0) * cos_w0)
            a2 = (A + 1.0) + (A - 1.0) * cos_w0 - s
        else:
            b0 = A * ((A + 1.0) + (A - 1.0) * cos_w0 + s)
            b1 = -2.0 * A * ((A - 1.0) + (A + 1.0) * cos_w0)
            b2 = A * ((A + 1.0) + (A - 1.0) * cos_w0 - s)
            a0 = (A + 1.0) - (A - 1.0) * cos_w0 + s
            a1 = 2.0 * ((A - 1.0) - (A + 1.0) * cos_w0)
            a2 = (A + 1.0) - (A - 1.0) * cos_w0 - s
    else:
        b0 = 1.0 + alpha * A
        b1 = -2.0 * cos_w0
        b2 = 1.0 - alpha * A
        a0 = 1.0 + alpha / A
        a1 = -2.0 * cos_w0
        a2 = 1.0 - alpha / A

    nb0, nb1, nb2 = b0 / a0, b1 / a0, b2 / a0
    na1, na2 = -(a1 / a0), -(a2 / a0)
    return [round(c * 1073741824.0) for c in (nb0, nb1, nb2, na1, na2)]


def _md8_build_write_packet(index: int, band: FilterBand) -> bytes:
    b = band.clamped()
    pkt = bytearray(64)
    pkt[0] = MD_REPORT_ID
    pkt[1] = MD_CMD_WRITE
    pkt[2] = MD_CMD_UPDATE_EQ
    pkt[3] = 0x18
    pkt[4] = 0x00
    pkt[5] = index
    pkt[6] = 0x00
    pkt[7] = 0x00

    coeffs = _md8_encode_biquad(b)
    for k, val in enumerate(coeffs):
        pkt[8 + k * 4 : 12 + k * 4] = int(val).to_bytes(4, "little", signed=True)

    freq_u16 = round(b.frequency)
    q_u16 = round(b.q * 256.0)
    gain_i16 = round(b.gain * 256.0)
    pkt[28:30] = freq_u16.to_bytes(2, "little")
    pkt[30:32] = q_u16.to_bytes(2, "little")
    pkt[32:34] = gain_i16.to_bytes(2, "little", signed=True)
    pkt[34] = _MD_TYPE_TO_CODE.get(b.type, 2)
    pkt[35] = 0
    pkt[36] = 7
    return bytes(pkt)


# --------------------------------------------------------------------------- #
# Device Transport
# --------------------------------------------------------------------------- #
class MoondropHidTransport:
    """Context manager wrapping an open /dev/hidrawX descriptor."""

    def __init__(self, info: Optional[MoondropDeviceInfo] = None) -> None:
        self.info = info or find_active_moondrop_device()
        if not self.info:
            raise Chu2HidError(
                "No se detectó ningún dispositivo Moondrop DSP por USB-C."
            )
        try:
            self._fd = os.open(self.info.hidraw_path, os.O_RDWR | os.O_NONBLOCK)
        except PermissionError as exc:
            raise Chu2HidError(
                f"Sin permisos para abrir {self.info.hidraw_path} ({self.info.display_name}, "
                f"{self.info.vid_pid}). Agrega una regla udev para idVendor={self.info.vid:04x}, "
                f"idProduct={self.info.pid:04x}."
            ) from exc
        except OSError as exc:
            raise Chu2HidError(f"Error abriendo {self.info.hidraw_path}: {exc}") from exc

    def close(self) -> None:
        if getattr(self, "_fd", -1) >= 0:
            try:
                os.close(self._fd)
            except OSError:
                pass
            self._fd = -1

    def __enter__(self) -> "MoondropHidTransport":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()

    def _drain_input(self) -> None:
        while True:
            r, _, _ = select.select([self._fd], [], [], 0.0)
            if not r:
                break
            try:
                os.read(self._fd, 64)
            except OSError:
                break

    def _kt_command(self, reg: int, cmd: int, value: bytes = bytes(4), ignore_disconnect: bool = False) -> bytes:
        pkt = _kt_packet(reg, cmd, value)
        for _attempt in range(2):
            self._drain_input()
            try:
                os.write(self._fd, pkt)
            except OSError as exc:
                raise Chu2HidError(f"Error escribiendo comando HID: {exc}") from exc

            for _ in range(8):
                r, _, _ = select.select([self._fd], [], [], 0.45)
                if not r:
                    break
                try:
                    reply = os.read(self._fd, 64)
                except OSError as exc:
                    if ignore_disconnect:
                        return bytes(4)
                    raise Chu2HidError(f"Error leyendo respuesta HID: {exc}") from exc
                if (
                    len(reply) >= 11
                    and reply[0] == KT_REPORT_ID
                    and reply[1] == reg
                    and reply[5] == cmd
                ):
                    return bytes(reply[7:11])
            if ignore_disconnect:
                return bytes(4)
        raise Chu2HidError(f"Sin respuesta del DSP para registro 0x{reg:02X} (cmd 0x{cmd:02X}).")

    def _md8_send_and_read(self, payload_with_id: bytes, expect_cmd0: int, expect_cmd1: int) -> bytes:
        self._drain_input()
        try:
            os.write(self._fd, payload_with_id)
        except OSError as exc:
            raise Chu2HidError(f"Error escribiendo comando HID Moondrop8: {exc}") from exc

        for _ in range(8):
            r, _, _ = select.select([self._fd], [], [], 0.6)
            if not r:
                break
            try:
                reply = os.read(self._fd, 64)
            except OSError as exc:
                raise Chu2HidError(f"Error leyendo respuesta HID Moondrop8: {exc}") from exc
            # Kernel hidraw prepends report ID 0x4B at reply[0]
            offset = 1 if (len(reply) > 1 and reply[0] == MD_REPORT_ID) else 0
            if len(reply) >= offset + 2 and reply[offset] == expect_cmd0 and reply[offset + 1] == expect_cmd1:
                return bytes(reply[offset:])
        raise Chu2HidError(f"Sin respuesta del DSP Moondrop (cmd 0x{expect_cmd1:02X}).")

    def read_eq(self) -> Tuple[bool, List[FilterBand]]:
        assert self.info is not None
        if self.info.protocol == "ktmicro":
            slot = self._kt_command(KT_REG_SLOT, KT_READ, bytes([KT_SLOT_ON, 0, 0, 0]))[0]
            bands: List[FilterBand] = []
            for i in range(self.info.max_bands):
                reg = KT_REG_BAND0 + 2 * i
                gf = self._kt_command(reg, KT_READ)
                qt = self._kt_command(reg + 1, KT_READ)
                bands.append(_kt_decode_band(gf, qt))
            return (slot != KT_SLOT_OFF), bands

        # moondrop8
        bands8: List[FilterBand] = []
        for i in range(self.info.max_bands):
            req = bytes([MD_REPORT_ID, MD_CMD_READ, MD_CMD_UPDATE_EQ, 0x18, 0x00, i, 0x00])
            data = self._md8_send_and_read(req, MD_CMD_READ, MD_CMD_UPDATE_EQ)
            if len(data) >= 34:
                freq = int.from_bytes(data[27:29], "little")
                q = int.from_bytes(data[29:31], "little") / 256.0
                gain = int.from_bytes(data[31:33], "little", signed=True) / 256.0
                ftype = _MD_CODE_TO_TYPE.get(data[33], "peaking")
                if 10 < freq < 24000:
                    bands8.append(FilterBand(type=ftype, frequency=float(freq), gain=gain, q=q).clamped())
                else:
                    bands8.append(FilterBand("peaking", 1000.0, 0.0, 1.0))
            else:
                bands8.append(FilterBand("peaking", 1000.0, 0.0, 1.0))
        return True, bands8

    def write_eq(self, bands: Sequence[FilterBand], commit: bool = False, enable_eq: bool = True) -> None:
        assert self.info is not None
        n = self.info.max_bands
        fitted: List[FilterBand] = []
        for b in list(bands)[:n]:
            bc = b.clamped()
            if not self.info.supports_shelves and bc.type != "peaking":
                bc.type = "peaking"
            bc.gain = float(round(min(self.info.max_gain_db, max(-self.info.max_gain_db, bc.gain)), 1))
            fitted.append(bc)
        while len(fitted) < n:
            fitted.append(FilterBand("peaking", 1000.0, 0.0, 1.0))

        if self.info.protocol == "ktmicro":
            slot = self._kt_command(KT_REG_SLOT, KT_READ, bytes([KT_SLOT_ON, 0, 0, 0]))[0]
            if enable_eq and slot == KT_SLOT_OFF:
                self._kt_command(KT_REG_SLOT, KT_WRITE, bytes([KT_SLOT_ON, 0, 0, 0]))
            elif not enable_eq and slot != KT_SLOT_OFF:
                self._kt_command(KT_REG_SLOT, KT_WRITE, bytes([KT_SLOT_OFF, 0, 0, 0]))

            for i, band in enumerate(fitted):
                reg = KT_REG_BAND0 + 2 * i
                gf, qt = _kt_encode_band(band)
                self._kt_command(reg, KT_WRITE, gf)
                self._kt_command(reg + 1, KT_WRITE, qt)
                if self._kt_command(reg, KT_READ) != gf or self._kt_command(reg + 1, KT_READ) != qt:
                    raise Chu2HidError(
                        f"Verificación fallida en banda {i + 1}: el DSP devolvió datos distintos. "
                        "No se guardó en memoria flash."
                    )

            if commit:
                self._kt_command(0x00, KT_COMMIT, ignore_disconnect=True)
            return

        # moondrop8
        for i, band in enumerate(fitted):
            os.write(self._fd, _md8_build_write_packet(i, band))
            enable_pkt = bytearray(64)
            enable_pkt[0] = MD_REPORT_ID
            enable_pkt[1] = MD_CMD_WRITE
            enable_pkt[2] = MD_CMD_APPLY_COEFF
            enable_pkt[3] = i
            enable_pkt[4] = 0
            enable_pkt[5] = 255
            enable_pkt[6] = 255
            enable_pkt[7] = 255
            os.write(self._fd, bytes(enable_pkt))
            time.sleep(0.015)

        if commit:
            os.write(self._fd, bytes([MD_REPORT_ID, MD_CMD_WRITE, MD_CMD_SAVE_FLASH]))


def read_chu2_eq(info: Optional[MoondropDeviceInfo] = None) -> Tuple[MoondropDeviceInfo, bool, List[FilterBand]]:
    """Open the active Moondrop DSP device, read active state & bands, and close."""
    with MoondropHidTransport(info) as dev:
        assert dev.info is not None
        enabled, bands = dev.read_eq()
        return dev.info, enabled, bands


def write_chu2_eq(
    bands: Sequence[FilterBand],
    commit: bool = False,
    enable_eq: bool = True,
    info: Optional[MoondropDeviceInfo] = None,
) -> MoondropDeviceInfo:
    """Write bands to the active Moondrop DSP device."""
    with MoondropHidTransport(info) as dev:
        assert dev.info is not None
        dev_info = dev.info
        dev.write_eq(bands, commit=commit, enable_eq=enable_eq)
    if commit and dev_info.disconnects_on_commit:
        deadline = time.monotonic() + 4.5
        time.sleep(0.8)
        while time.monotonic() < deadline:
            if find_active_moondrop_device():
                break
            time.sleep(0.25)
    return dev_info
