#!/usr/bin/env python3
"""AutoQ Standalone Linux AppImage Entrypoint.

- Launches the full Nothing OS 3-tab desktop app by default (works on GNOME, KDE,
  Hyprland, XFCE, X11, and Wayland without requiring any desktop widgets).
- Supports `--backend <args...>` for PyInstaller frozen subprocess execution.
- Supports optional widget installation from inside the AppImage:
    --install-hyprland-widget   Installs Hyprland widget + Waybar config
    --install-kde-plasmoid      Installs KDE Plasma 6 Plasmoid (org.hector.moondropeq)
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def _resource_root() -> Path:
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        return Path(sys._MEIPASS)
    return Path(__file__).resolve().parent.parent


def _install_hyprland_widget(root: Path) -> int:
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
    src = root / "widgets" / "linux-kde" / "plasmoid"
    if not src.exists():
        src = root / "linux-kde" / "plasmoid"
    if not src.exists():
        print("[ERROR] No se encontró el plasmoid KDE dentro del paquete.", file=sys.stderr)
        return 1
    if not shutil.which("kpackagetool6"):
        print("[ERROR] kpackagetool6 no está instalado (se requiere KDE Plasma 6).", file=sys.stderr)
        return 1
    # Try upgrade first, fall back to fresh install
    rc = subprocess.call(["kpackagetool6", "--type", "Plasma/Applet", "--upgrade", str(src)])
    if rc != 0:
        rc = subprocess.call(["kpackagetool6", "--type", "Plasma/Applet", "--install", str(src)])
    if rc == 0:
        print("[OK] Plasmoid AutoQ instalado en KDE Plasma 6.")
    return rc


def main() -> int:
    root = _resource_root()

    # 1. Internal backend dispatch for frozen PyInstaller builds
    if len(sys.argv) >= 2 and sys.argv[1] == "--backend":
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

    # Patch BackendWorker & status calls to use `--backend` when frozen
    if getattr(sys, "frozen", False):
        _orig_run = autoq_hyprland.BackendWorker.run

        def _frozen_run(self):  # type: ignore
            import json
            try:
                proc = subprocess.run(
                    [sys.executable, "--backend", *self.args],
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
                self.finished_json.emit(self.tag, json.loads(raw))
            except Exception as exc:
                self.finished_json.emit(self.tag, {"ok": False, "error": str(exc)})

        autoq_hyprland.BackendWorker.run = _frozen_run  # type: ignore

    # Default to --full standalone app unless user explicitly asks for --widget / --toggle
    if not any(f in sys.argv[1:] for f in ("--widget", "--full", "--toggle", "--status-waybar", "--help", "-h")):
        sys.argv.append("--full")

    return autoq_hyprland.main()


if __name__ == "__main__":
    raise SystemExit(main())
