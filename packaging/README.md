# Empaquetado nativo para gestores de paquetes Linux

AutoQ incluye soporte oficial para los principales ecosistemas de paquetes en Linux. Al instalarse con `pacman`/`paru` o `apt`, **las reglas USB HID (`/usr/lib/udev/rules.d/99-autoq-usb-dsp.rules`) se instalan automáticamente** sin que el usuario tenga que tocar archivos con `sudo`.

---

## 1. Arch Linux / CachyOS / EndeavourOS / Manjaro (`pacman` / `paru` / `yay`)

### Instalar localmente desde un clon del repo
```bash
cd packaging/aur
makepkg -si
```

### Publicar en el AUR para habilitar `paru -S autoq-bin` / `yay -S autoq-bin`
Arch Linux permite que cualquier mantenedor registre el paquete en `aur.archlinux.org`:
1. Sube tu llave SSH pública a tu cuenta en https://aur.archlinux.org.
2. Clona el repositorio vacío del paquete en AUR:
   ```bash
   git clone ssh://aur@aur.archlinux.org/autoq-bin.git
   cp packaging/aur/autoq-bin/PKGBUILD autoq-bin/
   cd autoq-bin
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m "Initial release: autoq-bin 1.1.0"
   git push origin master
   ```
3. A partir de ese momento, cualquier usuario de Arch / CachyOS puede instalarlo con:
   ```bash
   paru -S autoq-bin
   # o compilando desde código fuente Rust:
   paru -S autoq
   ```

---

## 2. Debian / Ubuntu / Linux Mint / Pop!_OS (`apt`)

El script [`packaging/deb/build_deb.sh`](./deb/build_deb.sh) genera un paquete `.deb` estándar (y GitHub Actions lo adjunta automáticamente en cada Release):

```bash
chmod +x packaging/deb/build_deb.sh
./packaging/deb/build_deb.sh 1.1.0
sudo apt install ./packaging/deb/dist/autoq_1.1.0_amd64.deb
```

* Instala `/usr/bin/autoq-cli` (motor Rust independiente) y `/usr/bin/autoq` (GUI Nothing OS).
* Incluye los widgets de Hyprland y KDE Plasma 6 dentro de `/usr/share/autoq/widgets/` para instalarlos en 1 clic desde la pestaña *Avanzado*.
* Instala y recarga automáticamente `/lib/udev/rules.d/99-autoq-usb-dsp.rules`.

---

## 3. Instalador universal de 1 línea (`Fedora`, `openSUSE`, `SteamOS`, servidores CLI)

```bash
# Instala autoq-cli + AutoQ AppImage en ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/Speed17x/AutoQ/main/install-linux.sh | bash

# O únicamente el motor CLI ligero en Rust (2.9 MB, cero dependencias gráficas):
curl -fsSL https://raw.githubusercontent.com/Speed17x/AutoQ/main/install-linux.sh | bash -s -- --cli-only
```
