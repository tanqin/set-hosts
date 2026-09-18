# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

A cross-platform hosts management tool (UI inspired by SwitchHosts), built with **Tauri 2 + Vue 3 + TypeScript + Rust**: multiple profiles, remote hosts subscriptions, append/overwrite write modes, automatic backup & restore, and proxy support for remote fetching. On desktop it writes the system hosts file with automatic privilege elevation (Windows UAC) and flushes the DNS cache; on mobile it takes over system DNS through a VPN consent so mappings take effect.

## Features

### Profile management

- **Local profiles**: create, rename, delete, edit hosts text, and enable / disable / apply with one click
- **Remote profiles**: subscribe to an `http(s)://` URL and fetch immediately on creation; name, URL and auto-refresh interval stay editable (changing the URL re-fetches at once and re-applies if enabled)
- **Scheduled auto refresh**: each remote profile has its own interval (never / 1 min / 5 min / 15 min / 1 h / 24 h / 7 d), polled every 30 seconds; refresh on startup can also be enabled
- **Merging**: entries of all enabled profiles are merged into a "managed block" in the system hosts file without conflicts

### Writing the system hosts file (desktop)

- **Append mode (default)**: entries are written into the managed block at the end of the hosts file, **keeping existing entries** (e.g. `127.0.0.1 localhost`)
- **Overwrite mode**: the enabled profiles' content **fully replaces** the hosts file (make sure `localhost` and others are part of a profile)
- The managed block is wrapped by `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<`; applying only adds/removes that block
- Automatic privilege elevation on Windows (UAC) and DNS cache flush after writing; macOS / Linux write to their standard hosts paths

### Safety & backup

- **Automatic backup before every write** to the system hosts file, keeping the latest 50 copies
- Manual backup, backup list, and one-click restore of any backup

### Built-in DNS proxy (mobile mapping)

Desktop rewrites the system hosts file directly; mobile cannot, so mappings take effect through a built-in local DNS server plus system VPN:

- **Fully automatic**: when a profile switch goes from off to on, the backend starts the local DNS server and requests system VPN consent — there is no related option in the UI
- **Listens on `127.0.0.1:5353`** by default (non-privileged port, no root/admin needed); if the port is taken it falls back to a random system port so the server always comes up
- **Mapped domains are answered directly** with `A` / `AAAA` and a TTL of 1 second (so toggling takes effect instantly); **everything else is forwarded** to the upstream DNS (UDP, with automatic TCP retry when the answer is truncated)
- **Upstream auto-detected**: the system DNS is detected automatically (`/etc/resolv.conf`, `ipconfig`); the DNS proxy has no UI option — the port and upstream are decided by the backend
- **Hot reload**: mappings update immediately after edits, toggles, remote refresh or import — no restart needed

### Proxy for remote fetching

- Remote hosts can be fetched through an **HTTP / HTTPS / SOCKS5** proxy, which only affects this app's fetch requests

### Import / Export

- **JSON**: full configuration (all profiles and backups) export / import
- **Hosts text**: export the current profile's raw hosts text, or import hosts text as a new profile
- Export to file / import from file

### App settings

- **UI language**: 简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) — 9 languages, switching takes effect instantly
- Theme: light / dark
- Hide to tray on startup, launch at login (desktop)
- Write mode, proxy for remote hosts fetching, auto refresh on startup
- Platform info: OS, desktop/mobile, hosts path (with one-click open of the containing folder)
- Custom data directory (movable)

## Tech stack

| Layer | Tech |
|---|---|
| Frontend | Vue 3 (`<script setup>`) + TypeScript + Pinia + Element Plus + Vite |
| Shell | Tauri 2 (tray-icon, dialog, shell, store, notification, autostart, single-instance plugins) |
| Backend | Rust: tokio, hickory-proto (DNS), reqwest (remote fetch; system TLS on desktop, rustls on mobile), serde, chrono, uuid |
| Mobile native | Android `VpnService` (`DnsVpnService.kt`, bridged to Rust via JNI) |

## Development

```bash
npm install                  # install dependencies
npm run tauri dev            # desktop dev mode
npm run dev                  # frontend only (no Tauri runtime, some features unavailable)
cd src-tauri && cargo test   # Rust unit tests
```

Mobile development:

```bash
npm run tauri android dev    # Android (JDK 17+ and Android SDK / NDK required)
npm run tauri ios dev        # iOS (macOS + Xcode required)
```

## Build & release

Build every target the current machine supports (desktop + Android, plus iOS on macOS; targets are independent):

```bash
npm run build:all
```

| Command | Description |
|---|---|
| `npm run build:check` | Check the build environment (JDK / Android SDK / NDK / rustup target) without building |
| `npm run build:desktop` | Desktop installer for the current OS (Windows `.exe` / `.msi`, macOS `.dmg`, Linux `.deb` / `.rpm` / `.AppImage`) |
| `npm run build:windows` / `build:macos` / `build:linux` | Explicit target OS; fails fast if it does not match the host |
| `npm run build:android` | Android APK (arm64 by default) |
| `npm run build:android:all` | All ABIs in one universal package |
| `npm run build:android:split` | One package per ABI (smaller) |
| `npm run build:android:debug` | Debug build (unsigned, debuggable) |
| `npm run build:android:aab` | AAB for Google Play |
| `npm run build:ios` | iOS (macOS + Xcode required) |

Artifacts are collected in the project root: `dist-desktop/`, `dist-apk/`, `dist-ios/`.

For finer control, run the scripts directly (on Windows PowerShell, `npm run xxx -- --flag value` swallows flags with values):

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Android build environment: JDK 17+ and the Android SDK (with NDK). The scripts auto-detect common install locations; you can also pass `--java-home` / `--sdk` or set `JAVA_HOME` / `ANDROID_HOME`. If a rustup target is missing, follow the hints from `npm run build:check` (e.g. `rustup target add aarch64-linux-android`).

### Version management

`package.json` is the single source of truth; `scripts/bump-version.mjs` syncs **8 places**: `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, Android `tauri.properties` (`versionName` / `versionCode`), the embedded Android `tauri.conf.json`, and iOS `project.pbxproj`.

| Command | Description |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | Bump version → sync all files → commit → create the `v*` annotated tag |
| `npm run version:sync` | No bump; sync every other file to the current `package.json` version |
| `npm run version:check` | Verify all places share the same version (non-zero exit on mismatch) |
| `npm run push` | `git push --follow-tags` — push commits and tags together |

The version shown on the About page is injected at build time from `package.json`, so every target always ships the latest version number.

## How it works

### Desktop: writing the system hosts file

1. Each profile stores raw hosts text (edited locally or fetched remotely)
2. On enable / disable / apply, the backend collects entries from **all enabled profiles**
3. New content is generated per write mode: append = strip the old managed block then append the new one; overwrite = keep only the new managed block
4. The current hosts file is backed up before the elevated write, then the DNS cache is flushed

### Mobile: built-in DNS proxy + VPN

1. All enabled profiles are compiled into a `domain → IP` map (first profile wins for duplicate domains)
2. The map is recompiled and hot-swapped whenever configuration changes
3. On a query: mapped domain → answer `A` / `AAAA` directly (empty answer when the family does not match, to avoid falling back to real DNS); unmapped or `CNAME` / `MX` etc. → forward upstream and return as-is
4. On Android, `DnsVpnService` points system DNS at the local server so mappings apply system-wide

## Platform support

| Platform | Status |
|---|---|
| Windows / macOS / Linux | ✅ Full support: writing system hosts, automatic backup, remote subscriptions, import/export |
| Android | ✅ Supported: built-in DNS proxy + `VpnService` takes over system DNS (the system VPN consent dialog appears once per package and is remembered) |
| iOS | 🚧 Partial: DNS proxy and frontend are done, the `Network Extension` tunnel is not wired up yet (requires a paid developer account and entitlement); until then mappings do not apply system-wide |

## Project layout

```
src/            Frontend (Vue 3 + TS): views, components, Pinia stores, i18n (9 languages)
src-tauri/      Rust backend: commands, hosts parsing, backup, DNS proxy, mobile native bridge
scripts/        Build scripts and version management script
dist-desktop/   Desktop installer artifacts
dist-apk/       Android artifacts
```

## Notes

- The first write on Windows triggers a UAC prompt — this is expected
- Overwrite mode removes existing hosts entries (including `localhost`); keep them inside a profile or restore from Backup & Restore
- Remote hosts content is limited to 8MB with a 15-second timeout; it must be plain hosts text (`IP domain` per line), not JSON
- On Android, enabling a profile for the first time requires granting the VPN consent; if denied the switch rolls back and consent is requested again next time
- The upstream DNS query timeout is 4 seconds; failures return `SERVFAIL`

## License

[Apache License 2.0](LICENSE) © 2026 tanqin
