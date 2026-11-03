# Building DropLink

## Prerequisites

| | Windows 10/11 x64 | macOS 11+ |
| --- | --- | --- |
| Node.js | 20 or newer | 20 or newer |
| Rust | stable via [rustup](https://rustup.rs) (`x86_64-pc-windows-msvc`) | stable via rustup |
| Native toolchain | Visual Studio 2022 Build Tools with *Desktop development with C++* | Xcode Command Line Tools (`xcode-select --install`) |
| Web runtime | WebView2 (preinstalled on Windows 11) | WKWebView (built in) |

Windows one-liners (as used for this project):

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## Development

```bash
npm install
npm run tauri dev        # Vite on :1420 + debug Rust build, hot reload for the UI
```

Logs go to the terminal and to the app log folder (Settings → Storage → Logs).
Set `DROPLINK_LOG=debug` for verbose engine logs.

## Release builds

```bash
npm run tauri build
```

Output in `target/release/bundle/`:

* **Windows**: `nsis/DropLink_1.0.0_x64-setup.exe` and `msi/DropLink_1.0.0_x64_en-US.msi`.
  Both add a firewall rule for private/domain networks when installed with
  administrator rights (`src-tauri/windows/hooks.nsh`, `firewall.wxs`).
* **macOS**: `macos/DropLink.app` and `dmg/DropLink_1.0.0_<arch>.dmg`.
  For one binary that runs on Apple Silicon and Intel:

  ```bash
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  npm run tauri build -- --target universal-apple-darwin
  ```

### Code signing

* **Windows**: set `bundle.windows.certificateThumbprint` (or use
  `signCommand`) in `src-tauri/tauri.conf.json` to sign the installer and
  binary; unsigned builds trigger SmartScreen warnings.
* **macOS**: export `APPLE_SIGNING_IDENTITY`, and for notarisation
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` before `tauri build`.

## Ports and firewall

DropLink listens on TCP **47100** (configurable in Settings → Network; the next
free port is used if it is taken) and uses mDNS on UDP 5353. For transfers
across networks, forward the TCP port or use a VPN.

## Project scripts

| Command | What it does |
| --- | --- |
| `npm run dev` | Vite dev server only (the UI needs the Tauri backend to do anything) |
| `npm run build` | Typecheck + production frontend build into `dist/` |
| `npm run typecheck` | TypeScript only |
| `npm test` | Vitest unit tests |
| `cargo test -p droplink-core` | Engine unit + integration tests |
| `cargo run -p droplink-core --example peer -- --accept-pairing` | Headless test peer (options in `crates/droplink-core/examples/peer.rs`) |
