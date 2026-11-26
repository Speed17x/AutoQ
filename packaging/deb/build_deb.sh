#!/usr/bin/env bash
set -euo pipefail

# Build a native Debian/Ubuntu package (`autoq_<version>_amd64.deb`)
# Installable via: sudo apt install ./autoq_1.1.0_amd64.deb
# Includes:
#   - /usr/bin/autoq-cli (native Rust engine, works headless too)
#   - /usr/bin/autoq (Nothing OS desktop app & widget launcher)
#   - /usr/share/autoq/widgets/* (bundled Hyprland & KDE Plasma 6 widgets)
#   - /lib/udev/rules.d/99-autoq-usb-dsp.rules (automatic USB HID permissions)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
VERSION="${1:-1.1.0}"
VERSION="${VERSION#v}"
ARCH="amd64"
PKG_NAME="autoq_${VERSION}_${ARCH}"
WORK_DIR="${SCRIPT_DIR}/build/${PKG_NAME}"
OUT_DIR="${SCRIPT_DIR}/dist"

echo "======================================================"
echo "  AutoQ — Generador de paquete Debian/Ubuntu (.deb)"
echo "  Versión: ${VERSION} (${ARCH})"
echo "======================================================"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/DEBIAN" "${OUT_DIR}"

RUST_CLI="${REPO_ROOT}/target/release/autoq-cli"
if [ ! -x "${RUST_CLI}" ]; then
    echo "[*] Compilando autoq-cli --release ..."
    (cd "${REPO_ROOT}" && cargo build --release -p autoq-cli)
fi

# 1. Populate filesystem tree
install -Dm755 "${RUST_CLI}" "${WORK_DIR}/usr/bin/autoq-cli"
install -Dm755 "${REPO_ROOT}/linux-hyprland/autoq_hyprland.py" "${WORK_DIR}/usr/share/autoq/widgets/linux-hyprland/autoq_hyprland.py"
install -Dm755 "${REPO_ROOT}/linux-hyprland/install.sh" "${WORK_DIR}/usr/share/autoq/widgets/linux-hyprland/install.sh"
install -Dm644 "${REPO_ROOT}/linux-hyprland/hyprland-autoq.conf" "${WORK_DIR}/usr/share/autoq/widgets/linux-hyprland/hyprland-autoq.conf"
install -Dm644 "${REPO_ROOT}/linux-hyprland/waybar-module.jsonc" "${WORK_DIR}/usr/share/autoq/widgets/linux-hyprland/waybar-module.jsonc"

mkdir -p "${WORK_DIR}/usr/share/autoq/widgets/linux-kde"
cp -a "${REPO_ROOT}/linux-kde/plasmoid" "${WORK_DIR}/usr/share/autoq/widgets/linux-kde/"

cat > "${WORK_DIR}/usr/bin/autoq" << 'EOF'
#!/usr/bin/env bash
export AUTOQ_CLI_BIN="/usr/bin/autoq-cli"
if [ $# -eq 0 ]; then
    exec python3 /usr/share/autoq/widgets/linux-hyprland/autoq_hyprland.py --full
else
    exec python3 /usr/share/autoq/widgets/linux-hyprland/autoq_hyprland.py "$@"
fi
EOF
chmod 755 "${WORK_DIR}/usr/bin/autoq"

install -Dm644 "${REPO_ROOT}/linux-appimage/autoq.desktop" "${WORK_DIR}/usr/share/applications/autoq.desktop"
install -Dm644 "${REPO_ROOT}/linux-appimage/autoq.svg" "${WORK_DIR}/usr/share/icons/hicolor/scalable/apps/autoq.svg"
install -Dm644 "${REPO_ROOT}/packaging/udev/99-autoq-usb-dsp.rules" "${WORK_DIR}/lib/udev/rules.d/99-autoq-usb-dsp.rules"

# 2. DEBIAN metadata & postinst udev reload
INSTALLED_KB="$(du -sk "${WORK_DIR}/usr" "${WORK_DIR}/lib" | awk '{s+=$1} END {print s}')"

cat > "${WORK_DIR}/DEBIAN/control" << EOF
Package: autoq
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_KB}
Depends: libc6, libgcc-s1
Recommends: python3, python3-pyside6.qtwidgets, pipewire, pulseaudio-utils
Maintainer: Speed17x <https://github.com/Speed17x/AutoQ>
Homepage: https://github.com/Speed17x/AutoQ
Description: Universal Parametric EQ & AutoEQ Suite (Nothing OS aesthetic)
 Native Rust DSP & USB-C HID engine (autoq-cli) supporting Moondrop 5/8-band
 hardware DSP, 10-band system-wide PipeWire EQ, and on-demand calibration
 across 14,500+ AutoEq & Squiglink headphone measurements.
EOF

cat > "${WORK_DIR}/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules >/dev/null 2>&1 || true
    udevadm trigger >/dev/null 2>&1 || true
fi
exit 0
EOF
chmod 755 "${WORK_DIR}/DEBIAN/postinst"

# 3. Build .deb (via dpkg-deb if available, or standard ar+tar fallback on Arch/Fedora)
OUT_DEB="${OUT_DIR}/${PKG_NAME}.deb"
rm -f "${OUT_DEB}"

if command -v dpkg-deb >/dev/null 2>&1; then
    dpkg-deb --build --root-owner-group "${WORK_DIR}" "${OUT_DEB}"
else
    TMP_AR="$(mktemp -d)"
    echo "2.0" > "${TMP_AR}/debian-binary"
    (cd "${WORK_DIR}/DEBIAN" && tar --owner=0 --group=0 -czf "${TMP_AR}/control.tar.gz" .)
    (cd "${WORK_DIR}" && tar --exclude='./DEBIAN' --owner=0 --group=0 -czf "${TMP_AR}/data.tar.gz" .)
    (cd "${TMP_AR}" && ar rcs "${OUT_DEB}" debian-binary control.tar.gz data.tar.gz)
    rm -rf "${TMP_AR}"
fi

echo "[OK] Paquete Debian generado en: ${OUT_DEB}"
echo "     Instalar en Debian/Ubuntu:  sudo apt install ./${PKG_NAME}.deb"
