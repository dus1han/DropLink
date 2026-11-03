# Platform notes and limitations

## Windows 10 / 11 (x64)

| Area | Implementation |
| --- | --- |
| Clipboard | `arboard` for text and images; `GetClipboardSequenceNumber` for cheap change detection; password-manager formats detected with `IsClipboardFormatAvailable`. |
| Discovery | `mdns-sd` (pure Rust, no Bonjour service required). |
| Firewall | NSIS and MSI installers add an inbound rule for `DropLink.exe` on private and domain profiles (requires installing with admin rights; otherwise Windows prompts on first launch). The rule is removed on uninstall. |
| Startup | `tauri-plugin-autostart` (registry Run key), launched with `--minimized`. |
| Notifications | `tauri-plugin-notification` (WinRT toasts). |
| Context menu | Optional **Send to → DropLink** shortcut in `%APPDATA%\Microsoft\Windows\SendTo` (Settings → General). Paths are forwarded to the running instance by `tauri-plugin-single-instance`. |
| Packaging | NSIS `.exe` (per-user or per-machine) and WiX `.msi`. |

Notes:
* If the network is marked **Public**, Windows blocks inbound connections and
  multicast; set it to Private for discovery to work.
* WebView2 is required (preinstalled on Windows 11 and current Windows 10).

## macOS 11+ (Apple Silicon and Intel)

| Area | Implementation |
| --- | --- |
| Clipboard | `arboard` (NSPasteboard). No change counter is exposed, so the watcher compares content hashes every 700 ms while sync is on; image reads are throttled to every ~3 s. |
| Discovery | `mdns-sd` over multicast. `Info.plist` declares `NSLocalNetworkUsageDescription` and `NSBonjourServices = _droplink._tcp` (required since macOS 15 — the system asks the user for Local Network permission on first use). |
| Startup | LaunchAgent via `tauri-plugin-autostart`. |
| Notifications | Native notification center (permission requested by the OS). |
| Window | Overlay title bar with traffic lights (`tauri.macos.conf.json`). |
| Files | Files can be dropped on the window. Tauri's `RunEvent::Opened` is handled, so files opened with DropLink are staged for sending. |
| Packaging | `.app` and `.dmg` via `npm run tauri build`. Build a universal binary with `--target universal-apple-darwin`. |

Limitations (documented, not hidden):
* `org.nspasteboard.ConcealedType` (used by some password managers to mark
  secrets) is not readable through `arboard`, so concealed items are **not**
  filtered on macOS yet. Automatic clipboard sync is off by default for this
  reason. A future native `NSPasteboard` backend (`objc2-app-kit`) can read
  `changeCount` and pasteboard types directly.
* A Finder **Share** / Services extension requires a separate signed app
  extension target and is not included.
* Distribution outside the App Store requires signing and notarisation with
  an Apple Developer ID (`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, … — see the
  Tauri docs). Unsigned builds run after right-click → Open.

## Linux

The core has no platform-specific code beyond the clipboard backend
(`arboard` supports X11 and Wayland). The Tauri shell builds on Linux, but
packaging and testing on Linux are out of scope for this release.

## Testing status per platform

* **Windows 11 x64**: the full automated suite (unit and integration tests
  over real TLS sockets, including a 1 GiB transfer) passes. The desktop app
  was built and launched, and mutual mDNS discovery with a second engine
  (the `peer` example) on the same machine was confirmed in the UI. Pairing
  and transfers through the GUI itself still need a click-through pass.
* **macOS**: not built or tested in this environment (no macOS machine was
  available). The code paths are platform-neutral except where noted above,
  and must be verified on a Mac before release — see DEVELOPMENT_STATUS.md.
