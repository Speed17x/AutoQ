#!/usr/bin/env bash
set -euo pipefail

# Build a 100% native Rust Debian/Ubuntu package (`autoq_<version>_amd64.deb`)
# ZERO Python / PySide6 dependencies!
# Installable via: sudo apt install ./autoq_2.0.0_amd64.deb

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
VERSION="${1:-2.0.0}"
VERSION="${VERSION#v}"
ARCH="amd64"
PKG_NAME="autoq_${VERSION}_${ARCH}"
WORK_DIR="${SCRIPT_DIR}/build/${PKG_NAME}"
OUT_DIR="${SCRIPT_DIR}/dist"

echo "============================================================"
echo "  AutoQ — Generador de paquete .deb 100% Rust (${VERSION})"
echo "============================================================"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/DEBIAN" "${OUT_DIR}"

RUST_GUI="${REPO_ROOT}/target/release/autoq-gui"
RUST_CLI="${REPO_ROOT}/target/release/autoq-cli"
if [ ! -x "${RUST_GUI}" ] || [ ! -x "${RUST_CLI}" ]; then
    echo "[*] Compilando autoq-gui y autoq-cli --release ..."
    (cd "${REPO_ROOT}" && cargo build --release --workspace)
fi

install -Dm755 "${RUST_GUI}" "${WORK_DIR}/usr/bin/autoq"
install -Dm755 "${RUST_CLI}" "${WORK_DIR}/usr/bin/autoq-cli"

install -Dm644 "${REPO_ROOT}/linux-hyprland/hyprland-autoq.conf" "${WORK_DIR}/usr/share/autoq/widgets/linux-hyprland/hyprland-autoq.conf"
install -Dm644 "${REPO_ROOT}/linux-hyprland/waybar-module.jsonc" "${WORK_DIR}/usr/share/autoq/widgets/linux-hyprland/waybar-module.jsonc"
mkdir -p "${WORK_DIR}/usr/share/autoq/widgets/linux-kde"
cp -a "${REPO_ROOT}/linux-kde/plasmoid" "${WORK_DIR}/usr/share/autoq/widgets/linux-kde/"

install -Dm644 "${REPO_ROOT}/linux-appimage/autoq.desktop" "${WORK_DIR}/usr/share/applications/autoq.desktop"
install -Dm644 "${REPO_ROOT}/linux-appimage/autoq.svg" "${WORK_DIR}/usr/share/icons/hicolor/scalable/apps/autoq.svg"
install -Dm644 "${REPO_ROOT}/packaging/udev/99-autoq-usb-dsp.rules" "${WORK_DIR}/lib/udev/rules.d/99-autoq-usb-dsp.rules"

INSTALLED_KB="$(du -sk "${WORK_DIR}/usr" "${WORK_DIR}/lib" | awk '{s+=$1} END {print s}')"

cat > "${WORK_DIR}/DEBIAN/control" << EOF
Package: autoq
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_KB}
Depends: libc6, libgcc-s1, libgl1, libxkbcommon0
Recommends: pipewire, pulseaudio-utils
Maintainer: Speed17x <https://github.com/Speed17x/AutoQ>
Homepage: https://github.com/Speed17x/AutoQ
Description: Universal Parametric EQ & AutoEQ Suite (100% Native Rust)
 Native Rust Nothing OS desktop app (autoq) and CLI (autoq-cli) with zero
 Python dependencies. Supports multiprotocol USB-C HID DSP (KTMicro 5B,
 Moondrop 8B, WalkPlay, FiiO 5/10B), 10-band PipeWire PEQ, and 14,500+
 AutoEq & Squiglink headphone curves.
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

echo "[OK] Paquete Debian 100% Rust generado en: ${OUT_DEB}"
ls -lh "${OUT_DEB}"
