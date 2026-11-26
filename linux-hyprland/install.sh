#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${HOME}/.local/share/autoq-hyprland"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"

echo "=========================================="
echo "  AutoQ — Nothing OS EQ para Hyprland"
echo "=========================================="

# 1. Verificar dependencias de Python
missing=()
for mod in PySide6 numpy requests; do
    if ! python3 -c "import ${mod}" >/dev/null 2>&1; then
        missing+=("${mod}")
    fi
done

if [ ${#missing[@]} -gt 0 ]; then
    echo ""
    echo "[!] Faltan módulos de Python: ${missing[*]}"
    echo "    En Arch / CachyOS / EndeavourOS instala con:"
    echo "      sudo pacman -S python-pyside6 python-numpy python-requests"
    echo "    En Fedora:"
    echo "      sudo dnf install python3-pyside6 python3-numpy python3-requests"
    echo "    O con pip:"
    echo "      pip install --user PySide6 numpy requests"
    echo ""
    read -r -p "¿Continuar copiando los archivos de todos modos? [S/n] " ans
    case "${ans:-S}" in
        [nN]*) exit 1 ;;
    esac
fi

# 2. Copiar aplicación y backend
mkdir -p "${INSTALL_DIR}/backend" "${BIN_DIR}" "${APP_DIR}"
cp -f "${SCRIPT_DIR}/autoq_hyprland.py" "${INSTALL_DIR}/"
cp -f "${SCRIPT_DIR}/hyprland-autoq.conf" "${INSTALL_DIR}/"
cp -f "${SCRIPT_DIR}/waybar-module.jsonc" "${INSTALL_DIR}/"
cp -f "${SCRIPT_DIR}/backend/"*.py "${INSTALL_DIR}/backend/"
chmod +x "${INSTALL_DIR}/autoq_hyprland.py" "${INSTALL_DIR}/backend/moondrop_backend.py"

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
