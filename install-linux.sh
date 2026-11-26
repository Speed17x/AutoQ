#!/usr/bin/env bash
set -euo pipefail

# Universal 1-line Linux installer for AutoQ (Rust CLI + Desktop AppImage)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Speed17x/AutoQ/main/install-linux.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/Speed17x/AutoQ/main/install-linux.sh | bash -s -- --cli-only

REPO="Speed17x/AutoQ"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
OPT_DIR="${HOME}/.local/share/autoq"
CLI_ONLY=0

for arg in "$@"; do
    case "${arg}" in
        --cli-only) CLI_ONLY=1 ;;
    esac
done

mkdir -p "${BIN_DIR}" "${APP_DIR}" "${OPT_DIR}"

echo "======================================================"
echo "  AutoQ — Instalador rápido para Linux (${REPO})"
echo "======================================================"

API_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
TAG="$(printf '%s' "${API_JSON}" | grep -m1 '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')"

if [ -z "${TAG}" ]; then
    echo "[ERROR] No se pudo consultar el último release de ${REPO}." >&2
    exit 1
fi
echo "[*] Última versión detectada: ${TAG}"

CLI_URL="https://github.com/${REPO}/releases/download/${TAG}/autoq-cli-linux-x86_64"
APPIMAGE_URL="https://github.com/${REPO}/releases/download/${TAG}/AutoQ-x86_64.AppImage"

if curl -fsSL -o "${BIN_DIR}/autoq-cli" "${CLI_URL}" 2>/dev/null; then
    chmod +x "${BIN_DIR}/autoq-cli"
    echo "[OK] Motor Rust CLI instalado en: ${BIN_DIR}/autoq-cli"
else
    echo "[i] Binario suelto autoq-cli aún no adjunto en ${TAG}; se usará el AppImage completo."
fi

if [ "${CLI_ONLY}" -eq 0 ]; then
    echo "[*] Descargando AutoQ-x86_64.AppImage ..."
    curl -fL -o "${OPT_DIR}/AutoQ-x86_64.AppImage" "${APPIMAGE_URL}"
    chmod +x "${OPT_DIR}/AutoQ-x86_64.AppImage"
    ln -sf "${OPT_DIR}/AutoQ-x86_64.AppImage" "${BIN_DIR}/autoq"

    cat > "${APP_DIR}/autoq.desktop" << EOF
[Desktop Entry]
Type=Application
Name=AutoQ
GenericName=Parametric EQ & AutoEQ
Comment=Universal Parametric EQ & AutoEQ Suite (Nothing OS style)
Exec=${BIN_DIR}/autoq
Icon=audio-headphones
Terminal=false
Categories=AudioVideo;Audio;Mixer;
StartupWMClass=autoq-widget
EOF
    echo "[OK] App de escritorio lista: ejecuta 'autoq' (o búscala en tu menú de aplicaciones)."
    echo "     Los widgets de Hyprland y KDE Plasma 6 se pueden instalar en 1 clic desde la pestaña AVANZADO."
fi

case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) echo "[!] Nota: agrega 'export PATH=\"\$HOME/.local/bin:\$PATH\"' a tu ~/.bashrc o ~/.zshrc." ;;
esac
