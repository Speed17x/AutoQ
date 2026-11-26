#!/usr/bin/env python3
"""AutoQ for Windows — Nothing OS Desktop App [BETA — UNTESTED].

Reuses the PySide6 Nothing OS interface while routing all hardware & EQ calls to
`backend/moondrop_backend_win.py` (hidapi USB-C HID + Equalizer APO 10-band EQ).
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


def _resource_root() -> Path:
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        return Path(sys._MEIPASS)
    return Path(__file__).resolve().parent


def main() -> int:
    root = _resource_root()

    # 1. Internal backend dispatch when frozen with PyInstaller
    if len(sys.argv) >= 2 and sys.argv[1] == "--backend":
        sys.path.insert(0, str(root / "backend"))
        import moondrop_backend_win  # type: ignore

        sys.argv = ["moondrop_backend_win.py", *sys.argv[2:]]
        return moondrop_backend_win.main()

    sys.path.insert(0, str(root))
    import autoq_ui  # type: ignore

    # Point backend to Windows backend (frozen or source)
    backend_py = root / "backend" / "moondrop_backend_win.py"
    autoq_ui.BACKEND_SCRIPT = backend_py

    def _win_backend_run(self):  # type: ignore
        try:
            cmd = (
                [sys.executable, "--backend", *self.args]
                if getattr(sys, "frozen", False)
                else [sys.executable, str(backend_py), *self.args]
            )
            creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=35,
                creationflags=creationflags,
            )
            raw = (proc.stdout or "").strip()
            if not raw:
                err = (proc.stderr or "").strip().splitlines()
                self.finished_json.emit(
                    self.tag,
                    {"ok": False, "error": err[0] if err else "Sin respuesta del backend Windows"},
                )
                return
            self.finished_json.emit(self.tag, json.loads(raw))
        except Exception as exc:
            self.finished_json.emit(self.tag, {"ok": False, "error": str(exc)})

    autoq_ui.BackendWorker.run = _win_backend_run  # type: ignore

    # Disable Linux AF_UNIX IPC on Windows
    autoq_ui.send_ipc_if_running = lambda *_a, **_kw: False  # type: ignore
    autoq_ui.start_ipc_server = lambda *_a, **_kw: None  # type: ignore

    from PySide6.QtWidgets import QApplication

    app = QApplication(sys.argv)
    app.setApplicationName("AutoQ (Windows Beta)")
    win = autoq_ui.AutoQNothingWindow(start_full=True)
    win.setWindowTitle("AutoQ — Windows BETA (Sin testear)")
    win.show()
    win.refresh_status(False)
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
