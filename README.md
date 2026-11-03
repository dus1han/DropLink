# DropLink

Send files, folders, text and clipboard contents between your Mac and Windows
computers — directly, encrypted end to end, with no cloud storage.

* **Nearby devices** appear automatically (mDNS); anything else can be reached
  by address (VPN, port forwarding).
* **Files and folders of any size**, streamed in chunks, verified with
  SHA-256, with pause, resume, cancel, retry and automatic recovery after
  network drops or restarts.
* **Several recipients at once**, each with independent progress and retry;
  **device groups** with their own permissions.
* **Universal clipboard** (text and images) with loop prevention and
  password-manager awareness, plus **instant text messages**.
* **Security first**: TLS 1.3 with mutual authentication, per-device key pairs,
  6-digit verified pairing, block/revoke, incoming approval, security log.

## Repository layout

```
crates/droplink-core   Rust engine: discovery, TLS, pairing, transfers, clipboard, SQLite
src-tauri              Tauri 2 shell: commands, notifications, tray, OS integration, packaging
src                    React + TypeScript UI (Vite, Tailwind v4, Framer Motion, Lucide)
docs                   Architecture, security model, platform notes, build guide
```

## Quick start (development)

Prerequisites: Node 20+, Rust (stable, via rustup), and the Tauri platform
prerequisites — on Windows the *Visual Studio C++ Build Tools* and WebView2,
on macOS the Xcode Command Line Tools. See [docs/BUILDING.md](docs/BUILDING.md).

```bash
npm install
npm run tauri dev
```

To try transfers on a single machine, run a headless second device next to
the app:

```bash
cargo run -p droplink-core --example peer -- --name "Test Peer" --accept-pairing
```

It shows up under **Nearby Devices**; pair with it and send files (saved to
`target/peer/downloads`).

## Tests

```bash
cargo test -p droplink-core                 # unit + integration tests (≈45 s)
cargo test -p droplink-core --release -- --ignored   # 1 GiB transfer
npm test                                    # frontend unit tests
npm run typecheck
```

## Production builds

```bash
npm run tauri build            # Windows: NSIS .exe + .msi   macOS: .app + .dmg
```

Details, signing and per-platform notes: [docs/BUILDING.md](docs/BUILDING.md).

## Documentation

* [Architecture](docs/ARCHITECTURE.md)
* [Security model](docs/SECURITY.md)
* [Platform notes and limitations](docs/PLATFORM_NOTES.md)
* [Development status](DEVELOPMENT_STATUS.md)

## Author

Developed by **Dushan Fernando** · © 2026 Dushan Fernando
