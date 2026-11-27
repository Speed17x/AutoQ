# Trivium — Linux AppImage (Universal Nativo Rust)

Empaquetado portable **AppImage (`x86_64`, `< 10 MB`)** de Trivium para cualquier distribución de Linux (Ubuntu, Fedora, Arch, CachyOS, Mint, Debian, etc.) y cualquier entorno de escritorio (GNOME, KDE, Hyprland, XFCE, Sway).

## Qué incluye
- **App de escritorio nativa en Rust (`trivium`)** con gráfica interactiva PEQ/FR, comparación A/B, control de Preamp + Salida (`-30..+12 dB`), 3 temas modulares y exportadores multi-formato.
- **Motor CLI (`trivium-cli`)** y **daemon opcional (`trivium-daemon`)** integrados en el mismo AppImage (`--backend` / `--daemon`).
- **Instaladores opcionales de widgets integrados**:
  - `--install-hyprland-widget`: instala el widget flotante de Hyprland + configuración de Waybar.
  - `--install-kde-plasmoid`: instala el Plasmoid nativo de KDE Plasma 6 (`org.speed17x.trivium`).

## Uso del AppImage

```bash
chmod +x Trivium-x86_64.AppImage

# Abrir la aplicación completa (funciona en cualquier escritorio)
./Trivium-x86_64.AppImage

# Abrir en modo widget compacto flotante
./Trivium-x86_64.AppImage --widget

# (Opcional) Instalar widget de Hyprland o Plasmoid KDE 6
./Trivium-x86_64.AppImage --install-hyprland-widget
./Trivium-x86_64.AppImage --install-kde-plasmoid
```

## Cómo compilar el AppImage localmente

Requisitos: toolchain de Rust (`cargo`), cabeceras de desarrollo (`libwayland-dev`, `libxkbcommon-dev`, `libgl1-mesa-dev`, `libegl1-mesa-dev`) y `librsvg2-bin` (`rsvg-convert`) opcional para el ícono PNG.

```bash
cd linux-appimage
chmod +x build_appimage.sh
./build_appimage.sh
```

El archivo final se genera en `linux-appimage/dist/Trivium-x86_64.AppImage`.
También se compila automáticamente en **GitHub Actions** ([`.github/workflows/build-releases.yml`](../.github/workflows/build-releases.yml)).
