#!/usr/bin/env python3
"""AutoQ Standalone Linux AppImage Entrypoint.

- Launches the full Nothing OS 3-tab desktop app by default (works on GNOME, KDE,
  Hyprland, XFCE, X11, and Wayland without requiring any desktop widgets).
- Routes all DSP / USB HID / PipeWire / AutoEQ calls to the native Rust `autoq-cli`
  binary bundled inside the AppImage (falling back to Python backend only in dev mode).
- Supports optional widget installation from inside the AppImage:
    --install-hyprland-widget   Installs Hyprland widget + Waybar config + ~/.local/bin/autoq-cli
    --install-kde-plasmoid      Installs KDE Plasma 6 Plasmoid + ~/.local/bin/autoq-cli
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional


def _resource_root() -> Path:
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        return Path(sys._MEIPASS)
    return Path(__file__).resolve().parent.parent


def _find_bundled_rust_cli(root: Path) -> Optional[Path]:
    candidates = [
        Path(sys.executable).resolve().parent / "autoq-cli",
        root / "autoq-cli",
        root / "backend" / "autoq-cli",
        root / "target" / "release" / "autoq-cli",
    ]
    for c in candidates:
        if c.is_file() and os.access(c, os.X_OK):
            return c
    return None


def _install_local_rust_cli(root: Path) -> None:
    cli = _find_bundled_rust_cli(root)
    if cli is not None:
        bin_dir = Path.home() / ".local" / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        dst = bin_dir / "autoq-cli"
        shutil.copy2(cli, dst)
        dst.chmod(0o755)
        print(f"[AutoQ] Motor Rust instalado en {dst}")


def _install_hyprland_widget(root: Path) -> int:
    _install_local_rust_cli(root)
    src = root / "widgets" / "linux-hyprland"
    if not src.exists():
        src = root / "linux-hyprland"
    installer = src / "install.sh"
    if not installer.exists():
        print("[ERROR] No se encontró linux-hyprland/install.sh dentro del paquete.", file=sys.stderr)
        return 1
    print(f"[AutoQ] Instalando widget de Hyprland desde {src} ...")
    return subprocess.call(["bash", str(installer)])


def _install_kde_plasmoid(root: Path) -> int:
    _install_local_rust_cli(root)
    src = root / "widgets" / "linux-kde" / "plasmoid"
    if not src.exists():
        src = root / "linux-kde" / "plasmoid"
    if not src.exists():
        print("[ERROR] No se encontró el plasmoid KDE dentro del paquete.", file=sys.stderr)
        return 1
    if not shutil.which("kpackagetool6"):
        print("[ERROR] kpackagetool6 no está instalado (se requiere KDE Plasma 6).", file=sys.stderr)
        return 1
    rc = subprocess.call(["kpackagetool6", "--type", "Plasma/Applet", "--upgrade", str(src)])
    if rc != 0:
        rc = subprocess.call(["kpackagetool6", "--type", "Plasma/Applet", "--install", str(src)])
    if rc == 0:
        print("[OK] Plasmoid AutoQ instalado en KDE Plasma 6 (conectado a autoq-cli en Rust).")
    return rc


def main() -> int:
    root = _resource_root()
    rust_cli = _find_bundled_rust_cli(root)
    if rust_cli is not None:
        os.environ["AUTOQ_CLI_BIN"] = str(rust_cli)

    # 1. Internal backend dispatch
    if len(sys.argv) >= 2 and sys.argv[1] == "--backend":
        if rust_cli is not None:
            return subprocess.call([str(rust_cli), *sys.argv[2:]])
        backend_dir = root / "backend"
        if not backend_dir.exists():
            backend_dir = root / "linux-hyprland" / "backend"
        sys.path.insert(0, str(backend_dir))
        import moondrop_backend  # type: ignore

        sys.argv = ["moondrop_backend.py", *sys.argv[2:]]
        return moondrop_backend.main()

    # 2. Optional desktop widget installers
    if "--install-hyprland-widget" in sys.argv:
        return _install_hyprland_widget(root)
    if "--install-kde-plasmoid" in sys.argv:
        return _install_kde_plasmoid(root)

    # 3. Launch full Nothing OS PySide6 desktop app
    hypr_dir = root / "linux-hyprland"
    if hypr_dir.exists():
        sys.path.insert(0, str(hypr_dir))
    sys.path.insert(0, str(root))

    import autoq_hyprland  # type: ignore

    # Default to --full standalone app unless user explicitly asks for --widget / --toggle
    if not any(f in sys.argv[1:] for f in ("--widget", "--full", "--toggle", "--status-waybar", "--help", "-h")):
        sys.argv.append("--full")

    return autoq_hyprland.main()


if __name__ == "__main__":
    raise SystemExit(main())
