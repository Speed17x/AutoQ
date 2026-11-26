#!/usr/bin/env bash
set -euo pipefail

# Build a self-contained AutoQ-x86_64.AppImage (Linux x86_64)
# Includes:
#   - Standalone Nothing OS PySide6 app (default: --full, works on any DE/WM)
#   - Optional bundled Hyprland widget and KDE Plasma 6 Plasmoid installers

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"
DIST_DIR="${SCRIPT_DIR}/dist"
APPDIR="${BUILD_DIR}/AutoQ.AppDir"

echo "=============================================="
echo "  AutoQ — Generador de AppImage (x86_64)"
echo "=============================================="

# 1. Check PyInstaller
if ! python3 -m PyInstaller --version >/dev/null 2>&1; then
    echo "[!] PyInstaller no está instalado. Instálalo con:"
    echo "      pip install pyinstaller PySide6 numpy requests"
    exit 1
fi

rm -rf "${BUILD_DIR}" "${DIST_DIR}"
mkdir -p "${BUILD_DIR}" "${DIST_DIR}"

# 2. Bundle Python app + backend + optional desktop widgets with PyInstaller (--onedir)
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
    --paths "${REPO_ROOT}/linux-hyprland/backend" \
    --hidden-import moondrop_backend \
    --hidden-import autoeq_db \
    --hidden-import biquad \
    --hidden-import chu2_hid \
    --hidden-import moondrop_bt_pw \
    --add-data "${REPO_ROOT}/linux-hyprland/backend:backend" \
    --add-data "${REPO_ROOT}/linux-hyprland:widgets/linux-hyprland" \
    --add-data "${REPO_ROOT}/linux-kde/plasmoid:widgets/linux-kde/plasmoid" \
    "${SCRIPT_DIR}/autoq_appimage_entry.py"

# 3. Assemble AppDir
mkdir -p "${APPDIR}/usr/bin" "${APPDIR}/usr/share/applications" "${APPDIR}/usr/share/icons/hicolor/scalable/apps"
cp -a "${BUILD_DIR}/pyi-dist/autoq/"* "${APPDIR}/usr/bin/"
cp "${SCRIPT_DIR}/autoq.desktop" "${APPDIR}/autoq.desktop"
cp "${SCRIPT_DIR}/autoq.desktop" "${APPDIR}/usr/share/applications/autoq.desktop"
cp "${SCRIPT_DIR}/autoq.svg" "${APPDIR}/autoq.svg"
cp "${SCRIPT_DIR}/autoq.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/autoq.svg"

cat > "${APPDIR}/AppRun" << 'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
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
