# Security & Code Signing Policy

## Supported Versions

| Version | Supported |
| :--- | :--- |
| `3.0.x` | ✅ Yes |
| `< 3.0.0` (legacy *AutoQ*) | ❌ No |

## Reporting a Vulnerability

If you discover a security vulnerability (for example in HID descriptor handling, PipeWire/Equalizer APO config generation, or IPC), please report it privately via GitHub Security Advisories:

- **Report URL:** https://github.com/Speed17x/Trivium/security/advisories/new

Please include reproduction steps, platform details, and affected version/commit. Do not open public issues for unpatched security vulnerabilities.

---

## Code Signing Policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

### Scope & Build Provenance
- **Signed Artifacts:** Windows release executables (`Trivium.exe`, `trivium-cli.exe`, `trivium-daemon.exe`) and the single-file Windows installer (`Trivium-Setup-x64.exe`).
- **Build Environment:** All signed artifacts are built exclusively from tagged release commits (`v*`) on the `main` branch inside clean, ephemeral **GitHub Actions** runners defined in [`.github/workflows/build-releases.yml`](./.github/workflows/build-releases.yml).
- **Verification:** Every published release also includes `SHA256SUMS.txt` generated directly in CI.

### Roles & Responsibilities
- **Project Owner / Maintainer / Approver:** [@Speed17x](https://github.com/Speed17x) — reviews all pull requests, triggers release tags, and approves signing requests.
- **No external third-party build machines** have access to signing credentials or release tokens.

---

## Privacy Policy

**Trivium** respects user privacy:
- **Zero telemetry or analytics:** Trivium does not collect, store, or transmit personal data, usage metrics, or device identifiers to any tracking server.
- **Network usage:** Network requests are strictly limited to downloading public frequency response measurements and target curves from open AutoEQ / Squiglink repositories upon user request (and optional manual catalog refreshes).
- **Local storage:** All presets, profiles, and UI settings remain locally on the user's device.
