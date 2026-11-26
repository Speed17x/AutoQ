#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
INSTALL_DIR="${HOME}/.local/share/autoq-hyprland"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"

echo "=================================================="
echo "  AutoQ — Nothing OS EQ para Hyprland (Rust)"
echo "=================================================="

mkdir -p "${INSTALL_DIR}" "${BIN_DIR}" "${APP_DIR}"

# 1. Compilar/instalar binarios nativos Rust (autoq-gui + autoq-cli)
if [ ! -x "${REPO_ROOT}/target/release/autoq-gui" ] || [ ! -x "${REPO_ROOT}/target/release/autoq-cli" ]; then
    if command -v cargo >/dev/null 2>&1 && [ -f "${REPO_ROOT}/Cargo.toml" ]; then
        echo "[*] Compilando AutoQ 100% Rust (autoq-gui + autoq-cli --release)..."
        (cd "${REPO_ROOT}" && cargo build --release --workspace)
    fi
fi

cp -f "${SCRIPT_DIR}/hyprland-autoq.conf" "${INSTALL_DIR}/"
cp -f "${SCRIPT_DIR}/waybar-module.jsonc" "${INSTALL_DIR}/"

if [ -x "${REPO_ROOT}/target/release/autoq-gui" ] && [ -x "${REPO_ROOT}/target/release/autoq-cli" ]; then
    cp -f "${REPO_ROOT}/target/release/autoq-gui" "${BIN_DIR}/autoq-widget"
    cp -f "${REPO_ROOT}/target/release/autoq-cli" "${BIN_DIR}/autoq-cli"
    chmod +x "${BIN_DIR}/autoq-widget" "${BIN_DIR}/autoq-cli"
    echo "[OK] Instalado widget 100% Rust nativo en: ${BIN_DIR}/autoq-widget"
    echo "[OK] Instalado motor CLI Rust en:         ${BIN_DIR}/autoq-cli"
else
    echo "[ERROR] Se requiere Cargo/Rust (https://rustup.rs) o descargar AutoQ-x86_64.AppImage desde Releases."
    exit 1
fi

# 2. Crear lanzador .desktop
cat > "${APP_DIR}/autoq-widget.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=AutoQ
GenericName=Parametric EQ & AutoEQ
Comment=Ecualizador paramétrico universal estilo Nothing OS (100% Rust Nativo)
Exec=${BIN_DIR}/autoq-widget --widget
Icon=audio-headphones
Terminal=false
Categories=AudioVideo;Audio;Mixer;
StartupWMClass=autoq-gui
EOF

echo ""
echo "Siguiente paso (Hyprland):"
echo "  1) Agrega esta línea a tu ~/.config/hypr/hyprland.conf:"
echo "       source = ~/.local/share/autoq-hyprland/hyprland-autoq.conf"
echo "  2) Recarga Hyprland y presiona SUPER + E (o ejecuta: autoq-widget)"
