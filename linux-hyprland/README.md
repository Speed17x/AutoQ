# AutoQ — Widget & App Estilo Nothing OS para Hyprland

Ecualizador paramétrico universal nativo para **Hyprland / Wayland** con estética **Nothing OS** (obsidiana mate, matriz de puntos 5×7, acento Nothing Red `#D71921`) y paridad completa con la app **AutoQ** de Android.

## Qué incluye
- **Widget flotante compacto (`--widget`)**: tarjeta squircle Nothing OS arrastrable, título en matriz de puntos 5×7, botón `ON`/`OFF` bypass, gráfica de curva EQ/FR en tiempo real sobre cuadrícula de puntos, carrusel de presets (`‹` / `›`) y botón rojo `EQ` para expandir a la app completa.
- **App completa de 3 pestañas (`--full`)**:
  - **EQ**: gráfica interactiva arrastrable con ratón (doble clic pone a `0 dB`, rueda ajusta `Q`), soporte para **5 / 8 bandas** en USB-C HID DSP (Moondrop Chu 2 DSP, May, etc.) y **10 bandas** virtuales en PipeWire para **cualquier** audífono Bluetooth, DAC USB-C genérico o salida 3.5 mm, filtros `PK` / `LS` / `HS`, modo `LIVE` y `FLASH` / `APLICAR PW`.
  - **AUTOQ**: catálogo integrado de **+14,500 curvas** sincronizado de **8 fuentes abiertas** (AutoEq GitHub + 7 bases Squiglink: *Super\* Review, ReganCipher, Aftersound, HBB, Paul Wasabii, Jaytiss, GadgetryTech*) — incluye *7Hz x Crinacle Zero 3*, *Moondrop Golden Ages 2*, *Space Travel 2*, Sony, Huawei, Apple, Nothing, Sennheiser y más. Incluye 8 curvas target (incluida *Harman Over-Ear 2018*) y modo **Imitar cualquier audífono**.
  - **AVANZADO**: presets universales sincronizados con el widget, importar/exportar **EqualizerAPO (`ParametricEQ.txt`)**, controles Bluetooth SPP (`POP/MON/BASS`, leer/escribir) cuando el audífono lo soporta y diagnóstico PipeWire.
- **Integración nativa con Hyprland + Waybar**: instancia única por socket Unix (`--toggle` para `SUPER+E`) y salida JSON lista para Waybar (`--status-waybar`).

## Instalación rápida (Arch / CachyOS / Hyprland)

1. Instala las dependencias (si no las tienes ya):
   ```bash
   sudo pacman -S python-pyside6 python-numpy python-requests pipewire wireplumber
   ```
2. Descomprime el paquete y ejecuta el instalador:
   ```bash
   tar -xzf AutoQ-Hyprland-NothingOS.tar.gz
   cd AutoQ-Hyprland
   chmod +x install.sh
   ./install.sh
   ```
3. Agrega esta línea al final de tu `~/.config/hypr/hyprland.conf`:
   ```ini
   source = ~/.local/share/autoq-hyprland/hyprland-autoq.conf
   ```
   *(Si usas Hyprland ≥ 0.53, revisa el bloque comentado en `hyprland-autoq.conf` con la nueva sintaxis `windowrule {}`.)*
4. Presiona **`SUPER + E`** para abrir/ocultar el widget Nothing OS (o **`SUPER + SHIFT + E`** para abrir directamente el modo completo).

## Waybar (opcional)
Revisa [`waybar-module.jsonc`](./waybar-module.jsonc) para agregar una píldora minimalista en tu barra con estado en vivo y clic para desplegar AutoQ.
