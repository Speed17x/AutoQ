#!/usr/bin/env python3
"""AutoQ for Windows — Nothing OS Desktop App [BETA — UNTESTED].

Reuses the PySide6 Nothing OS interface while routing all hardware & EQ calls to
the native Rust `autoq-cli.exe` engine (`hidapi` USB-C HID 5B/8B + Equalizer APO 10-band EQ),
falling back to `backend/moondrop_backend_win.py` only when `autoq-cli.exe` is absent.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import Optional


def _resource_root() -> Path:
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        return Path(sys._MEIPASS)
    return Path(__file__).resolve().parent


def _find_win_rust_cli(root: Path) -> Optional[Path]:
    candidates = [
        Path(sys.executable).resolve().parent / "autoq-cli.exe",
        root / "autoq-cli.exe",
        root / "backend" / "autoq-cli.exe",
        root.parent / "target" / "release" / "autoq-cli.exe",
    ]
    for c in candidates:
        if c.is_file():
            return c
    return None


def main() -> int:
    root = _resource_root()
    rust_cli = _find_win_rust_cli(root)
    if rust_cli is not None:
        os.environ["AUTOQ_CLI_BIN"] = str(rust_cli)

    # 1. Internal backend dispatch when frozen with PyInstaller
    if len(sys.argv) >= 2 and sys.argv[1] == "--backend":
        if rust_cli is not None:
            creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
            return subprocess.call([str(rust_cli), *sys.argv[2:]], creationflags=creationflags)
        sys.path.insert(0, str(root / "backend"))
        import moondrop_backend_win  # type: ignore

        sys.argv = ["moondrop_backend_win.py", *sys.argv[2:]]
        return moondrop_backend_win.main()

    sys.path.insert(0, str(root))
    import autoq_ui  # type: ignore

    backend_py = root / "backend" / "moondrop_backend_win.py"
    autoq_ui.BACKEND_SCRIPT = backend_py

    # Disable Linux AF_UNIX IPC on Windows
    autoq_ui.send_ipc_if_running = lambda *_a, **_kw: False  # type: ignore
    autoq_ui.start_ipc_server = lambda *_a, **_kw: None  # type: ignore

    from PySide6.QtWidgets import QApplication

    app = QApplication(sys.argv)
    app.setApplicationName("AutoQ (Windows Beta)")
    win = autoq_ui.AutoQNothingWindow(start_full=True)
    win.setWindowTitle("AutoQ — Windows BETA (Motor Rust)")
    win.show()
    win.refresh_status(False)
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
