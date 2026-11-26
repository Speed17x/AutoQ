"""Universal PipeWire audio sink detection (Bluetooth, generic USB, 3.5mm/analog),
10-band native PipeWire filter-chain PEQ, and optional Moondrop Bluetooth SPP RFCOMM."""

from __future__ import annotations

import dataclasses
import json
import os
import re
import shutil
import socket
import subprocess
import time
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

from biquad import FilterBand

CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "chu2-dsp-widget"
PW_CONF_FILE = CONFIG_DIR / "pw-moondrop-eq.conf"
BT_PROFILES_FILE = CONFIG_DIR / "bt_profiles.json"
PW_UNIT_NAME = "moondrop-pw-eq"
PW_SINK_NAME = "effect_input.moondrop_peq"
DEFAULT_VIRTUAL_BANDS = 10

_MOONDROP_BT_KEYWORDS = (
    "moondrop",
    "space travel",
    "golden ages",
    "nekocake",
    "moca",
    "robin",
    "marigold",
    "kuroneko",
    "ultrasonic",
    "pill",
    "edge",
    "block",
    "alice",
)

_BT_AUTOEQ_HINTS: List[Tuple[str, str]] = [
    ("space travel 2 ultra", "Moondrop Space Travel 2 Ultra"),
    ("space travel 2", "Moondrop Space Travel 2"),
    ("space travel", "Moondrop Space Travel"),
    ("golden ages 2", "Moondrop Golden Ages 2"),
    ("golden ages", "Moondrop Golden Ages"),
    ("moca", "Moondrop Moca"),
    ("nekocake", "Moondrop Nekocake"),
    ("edge", "Moondrop Edge"),
    ("block", "Moondrop BLOCK"),
    ("alice", "Moondrop Alice"),
]


@dataclasses.dataclass(frozen=True)
class PipeWireAudioDevice:
    sink_name: str
    mac: str
    display_name: str
    autoeq_hint: str
    kind: str = "bt_pipewire"        # "bt_pipewire" | "usb_pipewire" | "wired_pipewire"
    transport_badge: str = "BT·10B"
    supports_spp: bool = False
    max_bands: int = DEFAULT_VIRTUAL_BANDS
    max_gain_db: float = 12.0
    supports_shelves: bool = True


# Back-compat alias
BluetoothMoondropDevice = PipeWireAudioDevice


def _is_moondrop_bt_name(name: str) -> bool:
    low = name.lower()
    return any(k in low for k in _MOONDROP_BT_KEYWORDS)


def _infer_autoeq_hint(name: str) -> str:
    low = name.lower()
    for substr, hint in _BT_AUTOEQ_HINTS:
        if substr in low:
            return hint
    cleaned = re.sub(
        r"(?i)\b(le|ble|hands-free|ag|audio|stereo|est[eé]reo|anal[oó]gico|analog|headset|headphones|earbuds|controller|hd-audio)\b",
        "",
        name,
    )
    return re.sub(r"\s+", " ", cleaned).strip()


def _mac_from_sink_name(sink_name: str) -> str:
    m = re.search(r"bluez_output\.([0-9A-Fa-f_]{17})", sink_name)
    if m:
        return m.group(1).replace("_", ":").upper()
    return ""


def _run_cmd(args: List[str], timeout: float = 4.0) -> Tuple[int, str, str]:
    try:
        proc = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
        return proc.returncode, proc.stdout, proc.stderr
    except Exception as exc:
        return 1, "", str(exc)


def scan_pipewire_audio_sinks(usb_hid_vid_pids: Sequence[str] = ()) -> List[PipeWireAudioDevice]:
    """Detect all connected PipeWire sinks: any Bluetooth headphone/TWS, generic USB DACs,
    and analog/3.5mm wired outputs (excluding our own virtual EQ sink and already-claimed USB HID DACs)."""
    rc, out, _ = _run_cmd(["pactl", "-f", "json", "list", "sinks"])
    if rc != 0 or not out.strip():
        return []
    try:
        sinks = json.loads(out)
    except json.JSONDecodeError:
        return []

    hid_vp_set = {vp.lower() for vp in usb_hid_vid_pids if vp}
    found: List[PipeWireAudioDevice] = []

    for s in sinks:
        name = s.get("name", "")
        if not name or name == PW_SINK_NAME or "auto_null" in name or "dummy" in name.lower():
            continue
        props = s.get("properties", {}) or {}
        desc = (
            s.get("description")
            or props.get("device.description")
            or props.get("node.description")
            or props.get("device.alias")
            or name
        ).strip()

        if name.startswith("bluez_output."):
            mac = _mac_from_sink_name(name)
            found.append(
                PipeWireAudioDevice(
                    sink_name=name,
                    mac=mac,
                    display_name=desc,
                    autoeq_hint=_infer_autoeq_hint(desc),
                    kind="bt_pipewire",
                    transport_badge="BT·10B",
                    supports_spp=_is_moondrop_bt_name(desc) or _is_moondrop_bt_name(name),
                    max_bands=DEFAULT_VIRTUAL_BANDS,
                )
            )
            continue

        if name.startswith("alsa_output.usb-"):
            vid = str(props.get("device.vendor.id", "")).lower().replace("0x", "")
            pid = str(props.get("device.product.id", "")).lower().replace("0x", "")
            vp = f"{vid}:{pid}" if (vid and pid) else ""
            if vp and vp in hid_vp_set:
                continue
            if "31b2:0113" in hid_vp_set and ("chu2" in desc.lower() or "chu2" in name.lower()):
                continue
            found.append(
                PipeWireAudioDevice(
                    sink_name=name,
                    mac="",
                    display_name=desc,
                    autoeq_hint=_infer_autoeq_hint(desc),
                    kind="usb_pipewire",
                    transport_badge="USB·10B",
                    supports_spp=False,
                    max_bands=DEFAULT_VIRTUAL_BANDS,
                )
            )
            continue

        if name.startswith("alsa_output.") and "hdmi" not in name.lower():
            found.append(
                PipeWireAudioDevice(
                    sink_name=name,
                    mac="",
                    display_name=desc,
                    autoeq_hint="",
                    kind="wired_pipewire",
                    transport_badge="AUX·10B",
                    supports_spp=False,
                    max_bands=DEFAULT_VIRTUAL_BANDS,
                )
            )

    # Order Bluetooth first, then generic USB, then wired/analog
    order = {"bt_pipewire": 0, "usb_pipewire": 1, "wired_pipewire": 2}
    found.sort(key=lambda d: (order.get(d.kind, 9), d.display_name.lower()))
    return found


def scan_bluetooth_moondrop_sinks() -> List[PipeWireAudioDevice]:
    """Back-compat wrapper returning all detected PipeWire audio sinks."""
    return scan_pipewire_audio_sinks()


def is_pipewire_eq_running() -> bool:
    rc, out, _ = _run_cmd(["systemctl", "--user", "is-active", f"{PW_UNIT_NAME}.service"])
    return rc == 0 and out.strip() == "active"


def load_bt_saved_bands(key: str, count: int = DEFAULT_VIRTUAL_BANDS) -> List[FilterBand]:
    defaults = [31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0]
    if BT_PROFILES_FILE.exists():
        try:
            raw = json.loads(BT_PROFILES_FILE.read_text(encoding="utf-8"))
            items = raw.get(key)
            if isinstance(items, list) and items:
                bands = [FilterBand(**x).clamped() for x in items[:count]]
                while len(bands) < count:
                    bands.append(FilterBand("peaking", defaults[len(bands) % len(defaults)], 0.0, 1.0))
                return bands
        except Exception:
            pass
    return [FilterBand("peaking", defaults[i % len(defaults)], 0.0, 1.0) for i in range(count)]


def save_bt_bands(key: str, bands: Sequence[FilterBand]) -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    raw: Dict[str, list] = {}
    if BT_PROFILES_FILE.exists():
        try:
            raw = json.loads(BT_PROFILES_FILE.read_text(encoding="utf-8"))
        except Exception:
            raw = {}
    raw[key] = [vars(b.clamped()) for b in bands]
    BT_PROFILES_FILE.write_text(json.dumps(raw, indent=2, ensure_ascii=False), encoding="utf-8")


def _build_pipewire_conf(bands: Sequence[FilterBand], target_sink: str, label: str) -> str:
    label_map = {
        "peaking": "bq_peaking",
        "low_shelf": "bq_lowshelf",
        "high_shelf": "bq_highshelf",
    }
    nodes_lines: List[str] = []
    links_lines: List[str] = []
    active = [b.clamped() for b in bands]
    if not active:
        active = [FilterBand("peaking", 1000.0, 0.0, 1.0)]

    for i, b in enumerate(active, start=1):
        pw_label = label_map.get(b.type, "bq_peaking")
        nodes_lines.append(
            f'                    {{ type = builtin name = eq_band_{i} label = {pw_label} '
            f'control = {{ "Freq" = {b.frequency:.1f} "Q" = {b.q:.3f} "Gain" = {b.gain:.1f} }} }}'
        )
        if i > 1:
            links_lines.append(
                f'                    {{ output = "eq_band_{i - 1}:Out" input = "eq_band_{i}:In" }}'
            )

    nodes_block = "\n".join(nodes_lines)
    links_block = f"                links = [\n" + "\n".join(links_lines) + "\n                ]" if links_lines else ""

    safe_desc = label.replace('"', "")
    safe_target = target_sink.replace('"', "")
    return f"""context.properties = {{ log.level = 0 }}
context.spa-libs = {{
    audio.convert.* = audioconvert/libspa-audioconvert
    support.*       = support/libspa-support
}}
context.modules = [
    {{ name = libpipewire-module-protocol-native }}
    {{ name = libpipewire-module-client-node }}
    {{ name = libpipewire-module-adapter }}
    {{ name = libpipewire-module-filter-chain
        args = {{
            node.description = "AutoQ PEQ ({safe_desc})"
            media.name       = "AutoQ PEQ ({safe_desc})"
            filter.graph = {{
                nodes = [
{nodes_block}
                ]
{links_block}
            }}
            audio.channels = 2
            audio.position = [ FL FR ]
            capture.props = {{
                node.name   = "{PW_SINK_NAME}"
                media.class = Audio/Sink
            }}
            playback.props = {{
                node.name     = "effect_output.moondrop_peq"
                target.object = "{safe_target}"
                node.passive  = true
            }}
        }}
    }}
]
"""


def stop_pipewire_eq(restore_sink: Optional[str] = None) -> None:
    _run_cmd(["systemctl", "--user", "stop", f"{PW_UNIT_NAME}.service"])
    _run_cmd(["systemctl", "--user", "reset-failed", f"{PW_UNIT_NAME}.service"])
    if restore_sink:
        _run_cmd(["pactl", "set-default-sink", restore_sink])
        rc, out, _ = _run_cmd(["pactl", "-f", "json", "list", "sink-inputs"])
        if rc == 0 and out.strip():
            try:
                for si in json.loads(out):
                    _run_cmd(["pactl", "move-sink-input", str(si["index"]), restore_sink])
            except Exception:
                pass


def apply_pipewire_eq(dev: PipeWireAudioDevice, bands: Sequence[FilterBand], enable_eq: bool = True) -> bool:
    """Start or reload 10-band PipeWire filter-chain EQ targeting ``dev.sink_name``."""
    key = dev.mac or dev.sink_name or dev.display_name
    save_bt_bands(key, bands)

    if not enable_eq or all(abs(b.gain) < 0.05 for b in bands):
        stop_pipewire_eq(restore_sink=dev.sink_name)
        return False

    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    PW_CONF_FILE.write_text(_build_pipewire_conf(bands, dev.sink_name, dev.display_name), encoding="utf-8")

    _run_cmd(["systemctl", "--user", "stop", f"{PW_UNIT_NAME}.service"])
    _run_cmd(["systemctl", "--user", "reset-failed", f"{PW_UNIT_NAME}.service"])
    rc, _, err = _run_cmd(
        [
            "systemd-run",
            "--user",
            f"--unit={PW_UNIT_NAME}",
            "--description=AutoQ PipeWire PEQ Filter",
            "--collect",
            "pipewire",
            "-c",
            str(PW_CONF_FILE),
        ],
        timeout=5.0,
    )
    if rc != 0:
        raise RuntimeError(f"No se pudo iniciar PipeWire EQ: {err.strip()}")

    for _ in range(15):
        rc_s, out_s, _ = _run_cmd(["pactl", "list", "sinks", "short"])
        if rc_s == 0 and PW_SINK_NAME in out_s:
            break
        time.sleep(0.08)

    _run_cmd(["pactl", "set-default-sink", PW_SINK_NAME])
    rc_i, out_i, _ = _run_cmd(["pactl", "-f", "json", "list", "sink-inputs"])
    if rc_i == 0 and out_i.strip():
        try:
            for si in json.loads(out_i):
                _run_cmd(["pactl", "move-sink-input", str(si["index"]), PW_SINK_NAME])
        except Exception:
            pass
    return True


# --------------------------------------------------------------------------- #
# Optional Classic Bluetooth SPP (RFCOMM) for compatible Moondrop TWS
# --------------------------------------------------------------------------- #
def _resolve_spp_channels(mac: str) -> List[int]:
    channels: List[int] = []
    if shutil.which("sdptool"):
        rc, out, _ = _run_cmd(["sdptool", "browse", mac], timeout=6.0)
        if rc == 0:
            blocks = out.split("Service Name:")
            for blk in blocks:
                if "0x1101" in blk or "Serial Port" in blk:
                    m = re.search(r"Channel:\s*(\d+)", blk)
                    if m:
                        channels.append(int(m.group(1)))
    for ch in (1, 2, 3, 4, 5):
        if ch not in channels:
            channels.append(ch)
    return channels


def _spp_frame(payload: bytes) -> bytes:
    n = len(payload) - 4
    return bytes([0xFF, 0x04, (n >> 8) & 0xFF, n & 0xFF]) + payload


def _spp_request(mac: str, payload: bytes, timeout_s: float = 1.8) -> bytes:
    if not mac:
        raise RuntimeError("No hay dirección MAC Bluetooth para SPP.")
    last_err: Optional[Exception] = None
    sock: Optional[socket.socket] = None
    for ch in _resolve_spp_channels(mac):
        try:
            s = socket.socket(socket.AF_BLUETOOTH, socket.SOCK_STREAM, socket.BTPROTO_RFCOMM)
            s.settimeout(timeout_s)
            s.connect((mac, ch))
            sock = s
            break
        except OSError as exc:
            last_err = exc
            try:
                s.close()
            except Exception:
                pass
    if sock is None:
        raise RuntimeError(f"No se pudo conectar por Bluetooth SPP ({last_err}).")

    try:
        sock.sendall(_spp_frame(payload))
        want = payload[2] & 0xFE
        buf = bytearray()
        deadline = time.monotonic() + timeout_s
        while time.monotonic() < deadline:
            try:
                chunk = sock.recv(512)
            except socket.timeout:
                break
            if not chunk:
                break
            buf.extend(chunk)
            i = 0
            while i + 8 <= len(buf):
                if buf[i] == 0xFF and buf[i + 1] == 0x04:
                    ln = (buf[i + 2] << 8) | buf[i + 3]
                    total = 8 + ln
                    if len(buf) - i >= total:
                        frame = bytes(buf[i : i + total])
                        if (frame[6] & 0xFE) == want:
                            return frame
                        i += total
                        continue
                    break
                i += 1
        raise RuntimeError("Tiempo de espera agotado esperando respuesta Bluetooth SPP.")
    finally:
        sock.close()


def bt_spp_set_preset(mac: str, preset_idx: int) -> None:
    _spp_request(mac, bytes([0x00, 0x1D, 0x0A, 0x03, preset_idx & 0xFF]))


def bt_spp_read_eq(mac: str) -> List[FilterBand]:
    frame = _spp_request(mac, bytes([0x00, 0x1D, 0x0A, 0x05]))
    idx = frame.find(b"\x00\x04", 6)
    if idx < 0:
        raise RuntimeError("Este modelo Bluetooth no devolvió una curva PEQ personalizable por SPP.")
    p = frame[idx:]
    n = (len(p) - 4) // 7
    if len(p) < 11 or (len(p) - 4) % 7 != 0:
        raise RuntimeError("Formato EQ SPP no reconocido para este modelo.")

    def s16(o: int) -> int:
        return int.from_bytes(p[o : o + 2], "big", signed=True)

    def u16(o: int) -> int:
        return int.from_bytes(p[o : o + 2], "big", signed=False)

    bands: List[FilterBand] = []
    for i in range(n):
        o = 2 + i * 7
        g_raw = s16(o + 7) if i < n - 1 else s16(2 + n * 7)
        bands.append(
            FilterBand(
                type="peaking",
                frequency=float(u16(o + 2)),
                gain=round(g_raw / 60.0, 1),
                q=round(u16(o + 4) / 4096.0, 3),
            ).clamped()
        )
    return bands


def bt_spp_write_eq(mac: str, bands: Sequence[FilterBand]) -> None:
    five = [b.clamped() for b in list(bands)[:5]]
    while len(five) < 5:
        five.append(FilterBand("peaking", 1000.0, 0.0, 1.0))
    max_pos = max((b.gain for b in five if b.gain > 0), default=0.0)
    preamp = -max_pos

    out = bytearray(4 + len(five) * 7)
    out[0:2] = b"\x00\x04"

    def put_s16(o: int, val: float) -> None:
        r = max(-32768, min(32767, round(val * 60.0)))
        out[o : o + 2] = r.to_bytes(2, "big", signed=True)

    def put_u16(o: int, val: int) -> None:
        r = max(0, min(65535, val))
        out[o : o + 2] = r.to_bytes(2, "big", signed=False)

    put_s16(2, preamp)
    for i, b in enumerate(five):
        o = 2 + i * 7
        put_s16(o, preamp if i == 0 else five[i - 1].gain)
        put_u16(o + 2, round(b.frequency))
        put_u16(o + 4, round(b.q * 4096.0))
        out[o + 6] = 0
    put_s16(2 + len(five) * 7, five[-1].gain)

    for slot in (0x03, 0xFF):
        try:
            _spp_request(mac, bytes([0x00, 0x1D, 0x0A, 0x03, slot]), timeout_s=0.8)
            break
        except Exception:
            pass
    _spp_request(mac, bytes([0x00, 0x1D, 0x0A, 0x06]) + bytes(out), timeout_s=2.0)
