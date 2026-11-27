# Empaquetado nativo para gestores de paquetes y distribución

Trivium incluye empaquetado oficial **100 % nativo en Rust** para Linux, Windows, macOS ARM64 y Android. Al instalarse con `pacman`/`paru` o `apt`, **las reglas USB HID (`/usr/lib/udev/rules.d/99-trivium-usb-dsp.rules`, `MODE="0660", TAG+="uaccess"`) se instalan automáticamente** sin permisos abiertos innecesarios.

---

## 1. Arch Linux / CachyOS / EndeavourOS / Manjaro (`pacman` / `paru` / `yay`)

### Instalar localmente desde un clon del repo
```bash
cd packaging/aur
makepkg -si
```

### Publicar en el AUR para habilitar `paru -S trivium-bin` / `yay -S trivium-bin`
Arch Linux permite que cualquier mantenedor registre el paquete en `aur.archlinux.org`:
1. Sube tu llave SSH pública a tu cuenta en https://aur.archlinux.org.
2. Clona el repositorio vacío del paquete en AUR:
   ```bash
   git clone ssh://aur@aur.archlinux.org/trivium-bin.git
   cp packaging/aur/trivium-bin/PKGBUILD trivium-bin/
   cd trivium-bin
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m "Initial release: trivium-bin"
   git push origin master
   ```
3. A partir de ese momento, cualquier usuario de Arch / CachyOS puede instalarlo con:
   ```bash
   paru -S trivium-bin
   # o compilando desde código fuente Rust:
   paru -S trivium
   ```

---

## 2. Debian / Ubuntu / Linux Mint / Pop!_OS (`apt`)

El script [`packaging/deb/build_deb.sh`](./deb/build_deb.sh) genera un paquete `.deb` estándar (y GitHub Actions lo adjunta automáticamente en cada Release):

```bash
chmod +x packaging/deb/build_deb.sh
./packaging/deb/build_deb.sh 3.0.0
sudo apt install ./packaging/deb/dist/trivium_3.0.0_amd64.deb
```

* Instala `/usr/bin/trivium-cli` (motor Rust independiente), `/usr/bin/trivium` (GUI nativa Rust) y `/usr/bin/trivium-daemon` (daemon auto-perfil con unidad `systemd --user`).
* Incluye los widgets de Hyprland y KDE Plasma 6 dentro de `/usr/share/trivium/widgets/` para instalarlos en 1 clic desde la pestaña *Avanzado*.
* Instala y recarga automáticamente `/lib/udev/rules.d/99-trivium-usb-dsp.rules` (`MODE="0660", TAG+="uaccess"`).

---

## 3. Instalador universal de 1 línea para Linux (`x86_64` y `ARM64 / aarch64`)

Verifica automáticamente cada binario contra `SHA256SUMS.txt` antes de instalar:

```bash
# Instala AppImage/GUI + trivium-cli + trivium-daemon en ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/Speed17x/Trivium/main/install-linux.sh | bash

# O únicamente el motor CLI + daemon en Rust:
curl -fsSL https://raw.githubusercontent.com/Speed17x/Trivium/main/install-linux.sh | bash -s -- --cli-only
```

---

## 4. Windows WinGet & Android Obtainium

- **Windows WinGet**: consulta [`packaging/winget/README.md`](./winget/README.md) y la plantilla [`Speed17x.Trivium.yaml`](./winget/Speed17x.Trivium.yaml) para publicar `Trivium-Setup-x64.exe` en `microsoft/winget-pkgs` y solicitar firma Authenticode gratuita en **SignPath Foundation**.
- **Android Obtainium**: importa [`packaging/android/obtainium-trivium.json`](./android/obtainium-trivium.json) en la app **Obtainium** para actualizar el APK firmado en sitio.
