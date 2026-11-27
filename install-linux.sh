#!/usr/bin/env bash
set -euo pipefail

# Universal 1-line Linux installer for Trivium (Rust CLI + Desktop + Daemon)
# Verifies every downloaded binary against the release SHA256SUMS.txt.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Speed17x/Trivium/main/install-linux.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/Speed17x/Trivium/main/install-linux.sh | bash -s -- --cli-only

REPO="Speed17x/Trivium"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
OPT_DIR="${HOME}/.local/share/trivium"
CLI_ONLY=0

for arg in "$@"; do
    case "${arg}" in
        --cli-only) CLI_ONLY=1 ;;
    esac
done

RAW_ARCH="$(uname -m)"
case "${RAW_ARCH}" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)
        echo "[ERROR] Arquitectura no soportada por instalador binario: ${RAW_ARCH}." >&2
        echo "        Compila localmente con: cargo build --release --workspace" >&2
        exit 1
        ;;
esac

if ! command -v sha256sum >/dev/null 2>&1; then
    echo "[ERROR] Se requiere 'sha256sum' para verificar la integridad de la descarga." >&2
    exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

mkdir -p "${BIN_DIR}" "${APP_DIR}" "${OPT_DIR}"

echo "======================================================"
echo "  Trivium — Instalador rápido para Linux (${ARCH})"
echo "======================================================"

API_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
TAG="$(printf '%s' "${API_JSON}" | grep -m1 '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')"

if [ -z "${TAG}" ]; then
    echo "[ERROR] No se pudo consultar el último release de ${REPO}." >&2
    exit 1
fi
echo "[*] Última versión detectada: ${TAG}"

BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"
SUMS_FILE="${TMP_DIR}/SHA256SUMS.txt"

echo "[*] Descargando SHA256SUMS.txt para verificación de integridad..."
if ! curl -fsSL -o "${SUMS_FILE}" "${BASE_URL}/SHA256SUMS.txt"; then
    echo "[ERROR] No se encontró SHA256SUMS.txt en el release ${TAG}; instalación abortada por seguridad." >&2
    exit 1
fi

download_and_verify() {
    local asset_name="$1"
    local dest_path="$2"
    local expected_line
    expected_line="$(grep -E "^[0-9a-fA-F]{64}[[:space:]]+(\./)?${asset_name}$" "${SUMS_FILE}" | head -n1 || true)"
    if [ -z "${expected_line}" ]; then
        return 2
    fi
    local tmp_asset="${TMP_DIR}/${asset_name}"
    curl -fL --progress-bar -o "${tmp_asset}" "${BASE_URL}/${asset_name}" || return 1
    local expected_hash
    expected_hash="$(printf '%s' "${expected_line}" | awk '{print $1}')"
    if ! printf '%s  %s\n' "${expected_hash}" "${tmp_asset}" | sha256sum -c - >/dev/null 2>&1; then
        echo "[ERROR] Falló la verificación SHA-256 para ${asset_name}." >&2
        exit 1
    fi
    install -Dm755 "${tmp_asset}" "${dest_path}"
    return 0
}

CLI_ASSET="trivium-cli-linux-${ARCH}"
if download_and_verify "${CLI_ASSET}" "${BIN_DIR}/trivium-cli"; then
    echo "[OK] Motor Rust CLI verificado e instalado en: ${BIN_DIR}/trivium-cli"
else
    echo "[i] Binario ${CLI_ASSET} no disponible en ${TAG}."
fi

DAEMON_ASSET="trivium-daemon-linux-${ARCH}"
if download_and_verify "${DAEMON_ASSET}" "${BIN_DIR}/trivium-daemon"; then
    echo "[OK] Daemon auto-perfil verificado e instalado en: ${BIN_DIR}/trivium-daemon"
fi

if [ "${CLI_ONLY}" -eq 0 ]; then
    if [ "${ARCH}" = "x86_64" ]; then
        echo "[*] Descargando y verificando Trivium-x86_64.AppImage ..."
        if download_and_verify "Trivium-x86_64.AppImage" "${OPT_DIR}/Trivium-x86_64.AppImage"; then
            ln -sf "${OPT_DIR}/Trivium-x86_64.AppImage" "${BIN_DIR}/trivium"
        else
            echo "[ERROR] No se encontró Trivium-x86_64.AppImage en ${TAG}." >&2
            exit 1
        fi
    else
        echo "[*] Descargando y verificando trivium-linux-${ARCH} ..."
        if ! download_and_verify "trivium-linux-${ARCH}" "${BIN_DIR}/trivium"; then
            echo "[ERROR] No se encontró trivium-linux-${ARCH} en ${TAG}." >&2
            exit 1
        fi
    fi

    cat > "${APP_DIR}/trivium.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Trivium
GenericName=Parametric EQ & AutoEQ
Comment=Universal Parametric EQ & AutoEQ Suite (100% Native Rust)
Exec=${BIN_DIR}/trivium
Icon=audio-headphones
Terminal=false
Categories=AudioVideo;Audio;Mixer;
StartupWMClass=trivium
EOF
    echo "[OK] App de escritorio lista: ejecuta 'trivium' (o búscala en tu menú de aplicaciones)."
    echo "     Activa las reglas USB udev (0660 + uaccess) y trivium-daemon desde la pestaña AVANZADO."
fi

case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) echo "[!] Nota: agrega 'export PATH=\"\$HOME/.local/bin:\$PATH\"' a tu ~/.bashrc o ~/.zshrc." ;;
esac
