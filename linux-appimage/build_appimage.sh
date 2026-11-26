#!/usr/bin/env bash
set -euo pipefail

# Build a self-contained AutoQ-x86_64.AppImage (Linux x86_64)
# Includes:
#   - Native Rust `autoq-cli` engine (DSP math, USB HID 5B/8B, AutoEQ, PipeWire, SPP)
#   - Standalone Nothing OS PySide6 UI (zero numpy/requests bloat)
#   - Optional bundled Hyprland widget and KDE Plasma 6 Plasmoid installers

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"
DIST_DIR="${SCRIPT_DIR}/dist"
APPDIR="${BUILD_DIR}/AutoQ.AppDir"

echo "======================================================"
echo "  AutoQ — Generador de AppImage (Rust Core + Qt6 UI)"
echo "======================================================"

if ! command -v cargo >/dev/null 2>&1; then
    echo "[!] Rust/Cargo no está instalado. Instálalo desde https://rustup.rs"
    exit 1
fi

if ! python3 -m PyInstaller --version >/dev/null 2>&1; then
    echo "[!] PyInstaller no está instalado. Instálalo con:"
    echo "      pip install pyinstaller PySide6"
    exit 1
fi

rm -rf "${BUILD_DIR}" "${DIST_DIR}"
mkdir -p "${BUILD_DIR}" "${DIST_DIR}"

# 1. Compile native Rust backend (`autoq-cli`) in release mode
echo "[*] Compilando motor nativo Rust (autoq-cli --release) ..."
(cd "${REPO_ROOT}" && cargo build --release -p autoq-cli)
RUST_CLI="${REPO_ROOT}/target/release/autoq-cli"

# 2. Bundle lightweight PySide6 frontend + Rust `autoq-cli` (excluding heavy unused modules)
python3 -m PyInstaller \
    --noconfirm \
    --clean \
    --onedir \
    --windowed \
    --name autoq \
    --distpath "${BUILD_DIR}/pyi-dist" \
    --workpath "${BUILD_DIR}/pyi-work" \
    --specpath "${BUILD_DIR}" \
    --paths "${REPO_ROOT}/linux-hyprland" \
    --exclude-module numpy \
    --exclude-module requests \
    --exclude-module urllib3 \
    --exclude-module PySide6.QtWebEngineCore \
    --exclude-module PySide6.QtWebEngineWidgets \
    --exclude-module PySide6.Qt3DCore \
    --exclude-module PySide6.Qt3DRender \
    --exclude-module PySide6.QtMultimedia \
    --exclude-module PySide6.QtQml \
    --exclude-module PySide6.QtQuick \
    --exclude-module PySide6.QtSql \
    --exclude-module PySide6.QtPdf \
    --add-binary "${RUST_CLI}:." \
    --add-data "${REPO_ROOT}/linux-hyprland/autoq_hyprland.py:widgets/linux-hyprland" \
    --add-data "${REPO_ROOT}/linux-hyprland/install.sh:widgets/linux-hyprland" \
    --add-data "${REPO_ROOT}/linux-hyprland/hyprland-autoq.conf:widgets/linux-hyprland" \
    --add-data "${REPO_ROOT}/linux-hyprland/waybar-module.jsonc:widgets/linux-hyprland" \
    --add-data "${REPO_ROOT}/linux-kde/plasmoid:widgets/linux-kde/plasmoid" \
    "${SCRIPT_DIR}/autoq_appimage_entry.py"

# 3. Assemble AppDir
mkdir -p \
    "${APPDIR}/usr/bin" \
    "${APPDIR}/usr/share/applications" \
    "${APPDIR}/usr/share/icons/hicolor/scalable/apps" \
    "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

cp -a "${BUILD_DIR}/pyi-dist/autoq/"* "${APPDIR}/usr/bin/"
cp -f "${RUST_CLI}" "${APPDIR}/usr/bin/autoq-cli"
chmod +x "${APPDIR}/usr/bin/autoq-cli"

cp "${SCRIPT_DIR}/autoq.desktop" "${APPDIR}/autoq.desktop"
cp "${SCRIPT_DIR}/autoq.desktop" "${APPDIR}/usr/share/applications/autoq.desktop"
cp "${SCRIPT_DIR}/autoq.svg" "${APPDIR}/autoq.svg"
cp "${SCRIPT_DIR}/autoq.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/autoq.svg"

# Render 256x256 PNG icon (.DirIcon) using QtGui so appimagetool emits zero icon warnings
python3 - "${SCRIPT_DIR}/autoq.svg" "${APPDIR}/autoq.png" << 'PYEOF' || true
import sys
from PySide6.QtGui import QGuiApplication, QIcon, QImage, QPainter
from PySide6.QtCore import Qt
app = QGuiApplication([])
img = QImage(256, 256, QImage.Format.Format_ARGB32)
img.fill(Qt.GlobalColor.transparent)
icon = QIcon(sys.argv[1])
p = QPainter(img)
icon.paint(p, 0, 0, 256, 256)
p.end()
img.save(sys.argv[2], "PNG")
PYEOF

if [ -f "${APPDIR}/autoq.png" ]; then
    cp -f "${APPDIR}/autoq.png" "${APPDIR}/.DirIcon"
    cp -f "${APPDIR}/autoq.png" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/autoq.png"
fi

cat > "${APPDIR}/AppRun" << 'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export AUTOQ_CLI_BIN="${HERE}/usr/bin/autoq-cli"
exec "${HERE}/usr/bin/autoq" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

# 4. Locate or download appimagetool
APPIMAGETOOL="$(command -v appimagetool || true)"
if [ -z "${APPIMAGETOOL}" ]; then
    APPIMAGETOOL="${BUILD_DIR}/appimagetool-x86_64.AppImage"
    if [ ! -x "${APPIMAGETOOL}" ]; then
        echo "[*] Descargando appimagetool-x86_64.AppImage ..."
        curl -fSL -o "${APPIMAGETOOL}" \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
        chmod +x "${APPIMAGETOOL}"
    fi
fi

# 5. Build final AppImage
OUT_APPIMAGE="${DIST_DIR}/AutoQ-x86_64.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "${APPIMAGETOOL}" "${APPDIR}" "${OUT_APPIMAGE}"
chmod +x "${OUT_APPIMAGE}"

echo ""
echo "[OK] AppImage generado en: ${OUT_APPIMAGE}"
echo "     Ejecutar app completa:      ./AutoQ-x86_64.AppImage"
echo "     Instalar widget Hyprland:   ./AutoQ-x86_64.AppImage --install-hyprland-widget"
echo "     Instalar plasmoid KDE 6:    ./AutoQ-x86_64.AppImage --install-kde-plasmoid"
