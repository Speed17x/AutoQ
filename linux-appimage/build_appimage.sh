#!/usr/bin/env bash
set -euo pipefail

# Build a native Rust Trivium-x86_64.AppImage (< 10 MB)
# Includes:
#   - `trivium` (Native Rust desktop app + floating widget + 1-click installers)
#   - `trivium-cli` (Native Rust CLI + multiprotocol USB HID + Waybar + KDE Plasma backend)
#   - Bundled Hyprland config and KDE Plasma 6 Plasmoid (installable in 1 click)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"
DIST_DIR="${SCRIPT_DIR}/dist"
APPDIR="${BUILD_DIR}/Trivium.AppDir"

echo "============================================================"
echo "  Trivium — Generador de AppImage 100% Rust Nativo (x86_64)"
echo "============================================================"

if ! command -v cargo >/dev/null 2>&1; then
    echo "[!] Rust/Cargo no está instalado. Instálalo desde https://rustup.rs"
    exit 1
fi

rm -rf "${BUILD_DIR}" "${DIST_DIR}"
mkdir -p "${BUILD_DIR}" "${DIST_DIR}"

# 1. Compile native Rust GUI + CLI + daemon in release mode
echo "[*] Compilando trivium, trivium-cli y trivium-daemon en modo release ..."
(cd "${REPO_ROOT}" && cargo build --release --workspace)
RUST_GUI="${REPO_ROOT}/target/release/trivium"
RUST_CLI="${REPO_ROOT}/target/release/trivium-cli"
RUST_DAEMON="${REPO_ROOT}/target/release/trivium-daemon"

# 2. Assemble lightweight AppDir
mkdir -p \
    "${APPDIR}/usr/bin" \
    "${APPDIR}/usr/share/applications" \
    "${APPDIR}/usr/share/icons/hicolor/scalable/apps" \
    "${APPDIR}/usr/share/icons/hicolor/256x256/apps" \
    "${APPDIR}/usr/share/trivium/widgets/linux-hyprland" \
    "${APPDIR}/usr/share/trivium/widgets/linux-kde"

cp -f "${RUST_GUI}" "${APPDIR}/usr/bin/trivium"
cp -f "${RUST_CLI}" "${APPDIR}/usr/bin/trivium-cli"
cp -f "${RUST_DAEMON}" "${APPDIR}/usr/bin/trivium-daemon"
chmod +x "${APPDIR}/usr/bin/trivium" "${APPDIR}/usr/bin/trivium-cli" "${APPDIR}/usr/bin/trivium-daemon"

# Bundle optional desktop widgets (Hyprland + KDE Plasma 6)
cp -f "${REPO_ROOT}/linux-hyprland/hyprland-trivium.conf" "${APPDIR}/usr/share/trivium/widgets/linux-hyprland/"
cp -f "${REPO_ROOT}/linux-hyprland/waybar-module.jsonc" "${APPDIR}/usr/share/trivium/widgets/linux-hyprland/"
cp -a "${REPO_ROOT}/linux-kde/plasmoid" "${APPDIR}/usr/share/trivium/widgets/linux-kde/"

cp -f "${SCRIPT_DIR}/trivium.desktop" "${APPDIR}/trivium.desktop"
cp -f "${SCRIPT_DIR}/trivium.desktop" "${APPDIR}/usr/share/applications/trivium.desktop"
cp -f "${SCRIPT_DIR}/trivium.svg" "${APPDIR}/trivium.svg"
cp -f "${SCRIPT_DIR}/trivium.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/trivium.svg"

# Render 256x256 PNG icon (.DirIcon) if rsvg-convert or magick is present
if command -v rsvg-convert >/dev/null 2>&1; then
    rsvg-convert -w 256 -h 256 "${SCRIPT_DIR}/trivium.svg" -o "${APPDIR}/trivium.png"
elif command -v magick >/dev/null 2>&1; then
    magick -background none -resize 256x256 "${SCRIPT_DIR}/trivium.svg" "${APPDIR}/trivium.png"
fi

if [ -f "${APPDIR}/trivium.png" ]; then
    cp -f "${APPDIR}/trivium.png" "${APPDIR}/.DirIcon"
    cp -f "${APPDIR}/trivium.png" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/trivium.png"
fi

cat > "${APPDIR}/AppRun" << 'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
if [ "${1:-}" = "--backend" ]; then
    shift
    exec "${HERE}/usr/bin/trivium-cli" "$@"
fi
if [ "${1:-}" = "--daemon" ]; then
    shift
    exec "${HERE}/usr/bin/trivium-daemon" "$@"
fi
exec "${HERE}/usr/bin/trivium" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

# 3. Locate or download appimagetool
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

# 4. Build final ultra-light AppImage
OUT_APPIMAGE="${DIST_DIR}/Trivium-x86_64.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "${APPIMAGETOOL}" "${APPDIR}" "${OUT_APPIMAGE}"
chmod +x "${OUT_APPIMAGE}"

echo ""
echo "[OK] AppImage 100% Rust generado en: ${OUT_APPIMAGE}"
ls -lh "${OUT_APPIMAGE}"
