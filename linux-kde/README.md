# AutoQ para KDE Plasma 6 (Plasmoid QML + App Nativa Rust)

Implementación nativa para **KDE Plasma 6** compuesta por un **Plasmoid QML** (`org.hector.moondropeq`), el backend CLI ultrarrápido en Rust (`autoq-cli`) y la aplicación de escritorio estilo Nothing OS en Rust (`autoq-gui`).

## Componentes incluidos

| Archivo / Carpeta | Descripción |
| :--- | :--- |
| `plasmoid/` | Applet nativo de KDE Plasma 6 (`org.hector.moondropeq`, API Plasma 6.0) escrito en QML. |
| `moondrop_unity_pcm.sh` | Script auxiliar ALSA para fijar el mezclador hardware `PCM` al 100% (0 dB) en DACs `31b2:*` cuando se usa `soft-mixer` en WirePlumber. |

## Dependencias (Arch / CachyOS)
```bash
sudo pacman -S pipewire wireplumber cargo
```

## Instalación

### 1. Compilar binarios nativos Rust (`autoq-cli` y `autoq-gui`)
```bash
cargo build --release --workspace
install -Dm755 target/release/autoq-cli ~/.local/bin/autoq-cli
install -Dm755 target/release/autoq-gui ~/.local/bin/autoq
```

### 2. Instalar o actualizar el Plasmoid de KDE Plasma 6
```bash
cd linux-kde
kpackagetool6 --type Plasma/Applet --install plasmoid
# Si ya estaba instalado y quieres actualizarlo:
kpackagetool6 --type Plasma/Applet --upgrade plasmoid
```
También puedes instalarlo en 1 clic desde **AVANZADO → PLASMOID KDE 6** dentro de `autoq`.

## Configuración y caché
- Presets de usuario: `~/.config/autoq/presets.json`
- Reglas de auto-perfil (`autoq-daemon`): `~/.config/autoq/daemon_profiles.json`
- Caché de curvas AutoEq / Squiglink: `~/.cache/autoq/`
