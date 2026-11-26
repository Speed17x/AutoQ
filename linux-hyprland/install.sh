#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
INSTALL_DIR="${HOME}/.local/share/autoq-hyprland"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"

echo "=========================================="
echo "  AutoQ — Nothing OS EQ para Hyprland"
echo "=========================================="

mkdir -p "${INSTALL_DIR}/backend" "${BIN_DIR}" "${APP_DIR}"

# 1. Compilar o instalar el motor nativo en Rust (autoq-cli) si está disponible
RUST_BIN=""
if [ -x "${SCRIPT_DIR}/backend/autoq-cli" ]; then
    RUST_BIN="${SCRIPT_DIR}/backend/autoq-cli"
elif [ -x "${REPO_ROOT}/target/release/autoq-cli" ]; then
    RUST_BIN="${REPO_ROOT}/target/release/autoq-cli"
elif command -v cargo >/dev/null 2>&1 && [ -f "${REPO_ROOT}/Cargo.toml" ]; then
    echo "[*] Compilando motor nativo Rust (autoq-cli --release)..."
    (cd "${REPO_ROOT}" && cargo build --release -p autoq-cli)
    RUST_BIN="${REPO_ROOT}/target/release/autoq-cli"
fi

if [ -n "${RUST_BIN}" ] && [ -x "${RUST_BIN}" ]; then
    cp -f "${RUST_BIN}" "${BIN_DIR}/autoq-cli"
    cp -f "${RUST_BIN}" "${INSTALL_DIR}/backend/autoq-cli"
    chmod +x "${BIN_DIR}/autoq-cli" "${INSTALL_DIR}/backend/autoq-cli"
    echo "[OK] Motor Rust instalado en: ${BIN_DIR}/autoq-cli"
    PY_DEPS=(PySide6)
else
    echo "[!] autoq-cli (Rust) no encontrado; usando backend Python de respaldo."
    PY_DEPS=(PySide6 numpy requests)
fi

# 2. Verificar dependencias de Python (solo PySide6 cuando autoq-cli está activo)
missing=()
for mod in "${PY_DEPS[@]}"; do
    if ! python3 -c "import ${mod}" >/dev/null 2>&1; then
        missing+=("${mod}")
    fi
done

if [ ${#missing[@]} -gt 0 ]; then
    echo ""
    echo "[!] Faltan módulos de Python: ${missing[*]}"
    echo "    En Arch / CachyOS / EndeavourOS instala con:"
    echo "      sudo pacman -S pyside6"
    echo "    En Fedora:"
    echo "      sudo dnf install python3-pyside6"
    echo "    O con pip:"
    echo "      pip install --user ${missing[*]}"
    echo ""
    if [ -t 0 ]; then
        read -r -p "¿Continuar copiando los archivos de todos modos? [S/n] " ans
        case "${ans:-S}" in
            [nN]*) exit 1 ;;
        esac
    fi
fi

# 3. Copiar aplicación y backend
cp -f "${SCRIPT_DIR}/autoq_hyprland.py" "${INSTALL_DIR}/"
cp -f "${SCRIPT_DIR}/hyprland-autoq.conf" "${INSTALL_DIR}/"
cp -f "${SCRIPT_DIR}/waybar-module.jsonc" "${INSTALL_DIR}/"
if compgen -G "${SCRIPT_DIR}/backend/*.py" >/dev/null; then
    cp -f "${SCRIPT_DIR}/backend/"*.py "${INSTALL_DIR}/backend/"
    chmod +x "${INSTALL_DIR}/backend/moondrop_backend.py" 2>/dev/null || true
fi
chmod +x "${INSTALL_DIR}/autoq_hyprland.py"

# 3. Crear comando ejecutable ~/.local/bin/autoq-widget
cat > "${BIN_DIR}/autoq-widget" <<EOF
#!/usr/bin/env bash
exec python3 "${INSTALL_DIR}/autoq_hyprland.py" "\$@"
EOF
chmod +x "${BIN_DIR}/autoq-widget"

# 4. Crear lanzador .desktop (para rofi / wofi / walker / fuzzel)
cat > "${APP_DIR}/autoq-widget.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=AutoQ
GenericName=Parametric EQ & AutoEQ
Comment=Ecualizador paramétrico universal estilo Nothing OS (USB DSP / Bluetooth / PipeWire)
Exec=${BIN_DIR}/autoq-widget --widget
Icon=audio-headphones
Terminal=false
Categories=AudioVideo;Audio;Mixer;
StartupWMClass=autoq-widget
EOF

echo ""
echo "[OK] Instalado en: ${INSTALL_DIR}"
echo "[OK] Comando listo: ${BIN_DIR}/autoq-widget"
echo ""
echo "Siguiente paso (Hyprland):"
echo "  1) Agrega esta línea a tu ~/.config/hypr/hyprland.conf:"
echo "       source = ~/.local/share/autoq-hyprland/hyprland-autoq.conf"
echo "  2) Recarga Hyprland y presiona SUPER + E (o ejecuta: autoq-widget)"
echo ""
echo "Opcional — si usas audífonos USB-C con DSP integrado (Moondrop Chu 2 DSP, May, etc.),"
echo "crea /etc/udev/rules.d/99-autoq-usb-dsp.rules para escribir al chip sin sudo:"
echo '  SUBSYSTEM=="hidraw", ATTRS{idVendor}=="31b2", MODE="0666"'
echo '  SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2fc6", MODE="0666"'
echo '  SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2f20", MODE="0666"'
echo "  y ejecuta: sudo udevadm control --reload-rules && sudo udevadm trigger"
