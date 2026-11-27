# Trivium para KDE Plasma 6 (Plasmoid QML + App Nativa Rust)

Implementación nativa para **KDE Plasma 6** compuesta por un **Plasmoid QML** (`org.speed17x.trivium`), el backend CLI ultrarrápido en Rust (`trivium-cli`) y la aplicación de escritorio en Rust (`trivium`).

## Componentes incluidos

| Archivo / Carpeta | Descripción |
| :--- | :--- |
| `plasmoid/` | Applet nativo de KDE Plasma 6 (`org.speed17x.trivium`, API Plasma 6.0) escrito en QML. |
| `moondrop_unity_pcm.sh` | Script auxiliar ALSA para fijar el mezclador hardware `PCM` al 100% (0 dB) en DACs `31b2:*` cuando se usa `soft-mixer` en WirePlumber. |

## Dependencias (Arch / CachyOS)
```bash
sudo pacman -S pipewire wireplumber cargo
```

## Instalación

### 1. Compilar binarios nativos Rust (`trivium-cli` y `trivium`)
```bash
cargo build --release --workspace
install -Dm755 target/release/trivium-cli ~/.local/bin/trivium-cli
install -Dm755 target/release/trivium ~/.local/bin/trivium
```

### 2. Instalar o actualizar el Plasmoid de KDE Plasma 6
```bash
cd linux-kde
kpackagetool6 --type Plasma/Applet --install plasmoid
# Si ya estaba instalado y quieres actualizarlo:
kpackagetool6 --type Plasma/Applet --upgrade plasmoid
```
También puedes instalarlo en 1 clic desde **AVANZADO → PLASMOID KDE 6** dentro de `trivium`.

## Configuración y caché
- Presets de usuario: `~/.config/trivium/presets.json` (con migración automática desde `~/.config/autoq`)
- Reglas de auto-perfil (`trivium-daemon`): `~/.config/trivium/daemon_profiles.json`
- Caché de curvas AutoEq / Squiglink: `~/.cache/trivium/`
