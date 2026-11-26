# AutoQ — Widget & App Nativa en Rust para Hyprland / Wayland

Ecualizador paramétrico universal nativo (**100 % Rust**, `autoq-gui` + `autoq-cli`) para **Hyprland / Wayland** con temas modulares (**`Default`**, **`Default claro`** y **`NothingOS`**), selector de color de resaltado y paridad funcional completa con **AutoQ Android**.

## Qué incluye
- **Widget flotante compacto (`autoq --widget`)**: ventana compacta con gráfica de curva EQ/FR en tiempo real, botón `ON`/`OFF` bypass, carrusel de presets (`‹` / `›`) y botón `EQ` para expandir a la aplicación completa.
- **App completa de 3 pestañas (`autoq`)**:
  - **EQ**: gráfica interactiva arrastrable con ratón (doble clic pone a `0 dB`, rueda ajusta `Q`), comparación instantánea **A / B**, control manual de **Preamp + medidor anti-clipping**, soporte para **8 familias USB-C DSP** y **10 bandas** virtuales en PipeWire para Bluetooth, DAC USB genérico o 3.5 mm.
  - **AUTOQ**: catálogo integrado de **+14,500 curvas** sincronizado de **8 fuentes abiertas** (AutoEq GitHub + 7 bases Squiglink), 8 curvas objetivo curadas y modo **Imitar cualquier audífono**.
  - **AVANZADO**: temas modulares, presets sincronizados, importación APO y exportación multi-formato (**EqualizerAPO**, **EasyEffects**, **CamillaDSP** y **Wavelet**), controles Bluetooth SPP, instaladores 1-clic y gestión de `autoq-daemon`.
- **Integración nativa con Hyprland + Waybar**: instancia única por socket Unix (`--toggle` para `SUPER+E`) y salida JSON instantánea para Waybar (`--status-waybar`).

## Instalación rápida (Linux / Arch / CachyOS / Hyprland)

1. Instala las dependencias de audio del sistema (si no las tienes ya):
   ```bash
   sudo pacman -S pipewire wireplumber
   ```
2. Instala AutoQ desde el instalador oficial (o compila con `cargo build --release --workspace`):
   ```bash
   curl -fsSL https://raw.githubusercontent.com/Speed17x/AutoQ/main/install-linux.sh | bash
   ```
   Luego abre `autoq` y ve a la pestaña **AVANZADO → WIDGET HYPRLAND** (o ejecuta `autoq --install-hyprland-widget`), o bien instala manualmente desde este directorio:
   ```bash
   cd linux-hyprland
   chmod +x install.sh
   ./install.sh
   ```
3. Agrega esta línea al final de tu `~/.config/hypr/hyprland.conf`:
   ```ini
   source = ~/.local/share/autoq-hyprland/hyprland-autoq.conf
   ```
   *(Si usas Hyprland ≥ 0.53, revisa el bloque comentado en `hyprland-autoq.conf` con la nueva sintaxis `windowrule {}`.)*
4. Presiona **`SUPER + E`** para abrir/ocultar el widget (o **`SUPER + SHIFT + E`** para abrir directamente el modo completo).

## Waybar (opcional)
Revisa [`waybar-module.jsonc`](./waybar-module.jsonc) para agregar una píldora minimalista en tu barra con estado en vivo y clic para desplegar AutoQ.
