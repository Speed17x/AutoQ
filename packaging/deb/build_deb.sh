#!/usr/bin/env bash
set -euo pipefail

# Build a native Rust Debian/Ubuntu package (`trivium_<version>_amd64.deb`)
# Installable via: sudo apt install ./trivium_3.0.0_amd64.deb

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
VERSION="${1:-3.0.0}"
VERSION="${VERSION#v}"
ARCH="amd64"
PKG_NAME="trivium_${VERSION}_${ARCH}"
WORK_DIR="${SCRIPT_DIR}/build/${PKG_NAME}"
OUT_DIR="${SCRIPT_DIR}/dist"

echo "============================================================"
echo "  Trivium — Generador de paquete .deb 100% Rust (${VERSION})"
echo "============================================================"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/DEBIAN" "${OUT_DIR}"

RUST_GUI="${REPO_ROOT}/target/release/trivium"
RUST_CLI="${REPO_ROOT}/target/release/trivium-cli"
RUST_DAEMON="${REPO_ROOT}/target/release/trivium-daemon"
if [ ! -x "${RUST_GUI}" ] || [ ! -x "${RUST_CLI}" ] || [ ! -x "${RUST_DAEMON}" ]; then
    echo "[*] Compilando trivium, trivium-cli y trivium-daemon --release ..."
    (cd "${REPO_ROOT}" && cargo build --release --workspace)
fi

install -Dm755 "${RUST_GUI}" "${WORK_DIR}/usr/bin/trivium"
install -Dm755 "${RUST_CLI}" "${WORK_DIR}/usr/bin/trivium-cli"
install -Dm755 "${RUST_DAEMON}" "${WORK_DIR}/usr/bin/trivium-daemon"
install -Dm644 "${REPO_ROOT}/packaging/systemd/trivium-daemon.service" "${WORK_DIR}/usr/lib/systemd/user/trivium-daemon.service"

install -Dm644 "${REPO_ROOT}/linux-hyprland/hyprland-trivium.conf" "${WORK_DIR}/usr/share/trivium/widgets/linux-hyprland/hyprland-trivium.conf"
install -Dm644 "${REPO_ROOT}/linux-hyprland/waybar-module.jsonc" "${WORK_DIR}/usr/share/trivium/widgets/linux-hyprland/waybar-module.jsonc"
mkdir -p "${WORK_DIR}/usr/share/trivium/widgets/linux-kde"
cp -a "${REPO_ROOT}/linux-kde/plasmoid" "${WORK_DIR}/usr/share/trivium/widgets/linux-kde/"

install -Dm644 "${REPO_ROOT}/linux-appimage/trivium.desktop" "${WORK_DIR}/usr/share/applications/trivium.desktop"
install -Dm644 "${REPO_ROOT}/linux-appimage/trivium.svg" "${WORK_DIR}/usr/share/icons/hicolor/scalable/apps/trivium.svg"
install -Dm644 "${REPO_ROOT}/packaging/udev/99-trivium-usb-dsp.rules" "${WORK_DIR}/lib/udev/rules.d/99-trivium-usb-dsp.rules"

INSTALLED_KB="$(du -sk "${WORK_DIR}/usr" "${WORK_DIR}/lib" | awk '{s+=$1} END {print s}')"

cat > "${WORK_DIR}/DEBIAN/control" << EOF
Package: trivium
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_KB}
Depends: libc6, libgcc-s1, libgl1, libxkbcommon0
Recommends: pipewire, pulseaudio-utils
Maintainer: Speed17x <https://github.com/Speed17x/Trivium>
Homepage: https://github.com/Speed17x/Trivium
Description: Universal Parametric EQ & AutoEQ Suite (Native Rust)
 Native Rust desktop app (trivium), daemon (trivium-daemon) and CLI (trivium-cli).
 Supports 9 USB-C HID/Serial DSP families (KTMicro, Moondrop 8B, WalkPlay,
 FiiO, Qudelix-5K, Topping, Fosi DS3, JDS Element IV, Conexant), BLE GATT PEQ,
 10-band PipeWire PEQ, and 14,500+ AutoEq & Squiglink headphone curves.
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
