# Sidenotes — Pendientes (migración AutoQ → Trivium v3.0.0)

Notas de revisión local (2026-11-26). Marcar con `[x]` al completar.

## Migración / rename
- [x] Revisar ~936 menciones restantes de `autoq` (excluye `Cargo.lock`, `.git`, builds). Distinguir las intencionales (motor "AutoEQ", rutas de migración) de las que son nombres sin renombrar (`main.qml`, `trivium-gui/src/main.rs`, temas Kotlin UI, `.gitignore`, docs).
- [x] Verificar migración de configuración antiguo → nuevo (`~/.config/autoq` → `~/.config/trivium` mediante marcador `.migrated_from_autoq` + test unitario en `crates/trivium-core/src/config.rs`).
- [x] Android: el cambio de `applicationId` (`com.hector.moondropdsp` → `com.speed17x.trivium`) rompe la actualización in-place del APK firmado. Documentado en [`android/README.md`](./android/README.md) junto con la entrada limpia en Obtainium (`packaging/android/obtainium-trivium.json`).
- [x] Hacer commit de la migración en una rama (`feat/trivium-v3`).

## Calidad / mantenimiento
- [x] Tests: añadidos tests *golden-frame* de protocolo HID (KTMicro, Moondrop 8B, WalkPlay, Conexant, FiiO, Fosi DS3, Topping CRC-16, Qudelix V3) en [`crates/trivium-core/src/hid_tests.rs`](./crates/trivium-core/src/hid_tests.rs).
- [ ] Dividir archivos grandes: `trivium-cli` y `hid_tests.rs` ya están modularizados; pendientes `trivium-gui/src/main.rs` (~2441 líneas) y submódulos internos de `hid.rs`.
- [x] Matriz de dispositivos: marcar cuáles están probados en hardware real (✅) y cuáles solo implementados por protocolo (🧪) en [`docs/HARDWARE_MATRIX.md`](./docs/HARDWARE_MATRIX.md).
- [x] CI: añadir workflow de `cargo test`/`clippy` y tests Android ([`.github/workflows/ci.yml`](./.github/workflows/ci.yml)).

## Documentación / distribución
- [x] Acortar el README (resumen arriba, detalle en `docs/`).
- [x] Versión en inglés de la documentación ([`README.md`](./README.md) en inglés, [`README.es.md`](./README.es.md) en español, [`docs/HARDWARE_MATRIX.md`](./docs/HARDWARE_MATRIX.md) en inglés y enlace de traducción automática).
- [ ] Firma Authenticode pública en Windows (SmartScreen).
