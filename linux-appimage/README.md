# AutoQ — Linux AppImage (Universal)

Empaquetado portable **AppImage (`x86_64`)** de AutoQ para cualquier distribución de Linux (Ubuntu, Fedora, Arch, CachyOS, Mint, Debian, etc.) y cualquier entorno de escritorio (GNOME, KDE, Hyprland, XFCE, Sway).

## Qué incluye
- **App de escritorio independiente** (modo completo de 3 pestañas estilo Nothing OS por defecto, sin requerir widgets ni configuración de escritorio).
- **Instaladores opcionales de widgets integrados** en el mismo binario:
  - `--install-hyprland-widget`: instala el widget flotante de Hyprland + configuración de Waybar.
  - `--install-kde-plasmoid`: instala el Plasmoid nativo de KDE Plasma 6 (`org.hector.moondropeq`).

## Uso del AppImage

```bash
chmod +x AutoQ-x86_64.AppImage

# Abrir la aplicación completa (funciona en cualquier escritorio)
./AutoQ-x86_64.AppImage

# Abrir en modo widget compacto flotante
./AutoQ-x86_64.AppImage --widget

# (Opcional) Instalar widget de Hyprland
./AutoQ-x86_64.AppImage --install-hyprland-widget

# (Opcional) Instalar Plasmoid de KDE Plasma 6
./AutoQ-x86_64.AppImage --install-kde-plasmoid
```

## Cómo compilar el AppImage localmente

```bash
pip install pyinstaller PySide6 numpy requests
cd linux-appimage
chmod +x build_appimage.sh
./build_appimage.sh
```

El archivo final se genera en `linux-appimage/dist/AutoQ-x86_64.AppImage`.
También se compila automáticamente en **GitHub Actions** (`.github/workflows/build-releases.yml`).
