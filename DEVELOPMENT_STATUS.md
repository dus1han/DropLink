# Development status

Last updated: 2026-11-03 · Version 1.0.0

Legend: ✅ implemented and tested · 🟡 implemented, verified only partially (see note) · ⬜ not implemented

## Milestone 1 — Foundation, design system, shell ✅

| Item | Status | Notes |
| --- | --- | --- |
| Project setup (Tauri 2, React 19, TS, Vite, Tailwind v4, Framer Motion, Lucide, Zustand) | ✅ | `npm run build` and `cargo build` pass |
| Design system (tokens, dark + light themes, primitives) | ✅ | `src/styles/index.css`, `src/components/ui` |
| Desktop shell: collapsible sidebar, identity footer, top bar (network, discovery, notifications, theme) | ✅ | |
| Overview dashboard, device cards, quick actions | ✅ | Live stats from SQLite |

## Milestone 2 — Networking, discovery, pairing ✅

| Item | Status | Notes |
| --- | --- | --- |
| Device identity (ECDSA P-256, SHA-256 fingerprint) | ✅ | unit test |
| TLS 1.3 mutual authentication, hello binding | ✅ | integration tests incl. impersonation attempt |
| mDNS advertise/browse, join/leave, dedup | 🟡 | registry dedup unit-tested; mDNS exercised manually on Windows (see "Verification log") |
| Only verified DropLink devices are listed | ✅ | an mDNS record is shown only after a TLS + DropLink handshake whose certificate matches the advertised ID |
| Manual connection by address | ✅ | used by every integration test |
| SAS pairing (commit–reveal, both confirm) | ✅ | accept, decline, code equality tested |
| Block / revoke / forget, security log | ✅ | tested |
| Device renaming (local alias) | ✅ | |

## Milestone 3 — Transfer engine ✅

| Item | Status | Notes |
| --- | --- | --- |
| Chunked streaming (256 KiB), constant memory | ✅ | 96 MiB test; 1 GiB test (≈387 MB/s over loopback TLS, release build) |
| Files, multiple files, folders (hierarchy, empty dirs) | ✅ | tested |
| SHA-256 end-to-end verification, corrupt data discarded | ✅ | tested with a malicious sender |
| Timestamps preserved | ✅ | tested |
| Conflict handling (keep both / replace), destination choice at approval | ✅ | tested |
| Incoming approval, per-device and group auto-accept | ✅ | tested |
| Path traversal protection | ✅ | tested |
| Progress events (speed, ETA, current file) | ✅ | |

## Milestone 4 — Multi-device, groups, queue, resume ✅

| Item | Status | Notes |
| --- | --- | --- |
| Multiple recipients, independent progress/failure/retry | ✅ | tested (one recipient failing, retried alone) |
| Device groups: CRUD, members, permissions, send to group | ✅ | tested (untrusted members excluded) |
| Concurrency limit queue, bandwidth limit | ✅ | tested |
| Pause / resume from partial data | ✅ | tested |
| Recovery after receiver restart (auto-resume on reconnect) | ✅ | tested |
| Cancel discards partial data on both sides | ✅ | tested |

## Milestone 5 — Clipboard and text ✅

| Item | Status | Notes |
| --- | --- | --- |
| Two-way text sync, loop prevention | ✅ | tested with in-memory clipboards |
| Image sync (PNG on the wire) | ✅ | tested |
| Per-device and group permissions, global auto-sync toggle, manual send | ✅ | tested |
| Sensitive content skipped | 🟡 | Windows formats implemented; logic tested; macOS concealed type not detectable (PLATFORM_NOTES) |
| Clipboard history | ✅ | |
| Text sharing (multi-recipient, formatting preserved, copy, save as TXT) | ✅ | tested |

## Milestone 6 — History, notifications, settings, security UI ✅

| Item | Status | Notes |
| --- | --- | --- |
| Searchable history with date/device/status/direction filters, retry, clear | ✅ | SQL filters unit-tested (incl. LIKE escaping) |
| Native notifications with per-type preferences, in-app notification center | 🟡 | implemented; Windows toast delivery depends on OS notification settings |
| Settings (general, network, transfers, clipboard, security, notifications, storage) | ✅ | |
| Tray / background mode, launch at login, Explorer "Send to" | 🟡 | implemented; checked manually on Windows |

## Milestone 7 — Cross-platform, packaging 🟡

| Item | Status | Notes |
| --- | --- | --- |
| Windows build + NSIS/MSI with firewall rule | 🟡 | see verification log |
| macOS build (.app/.dmg, Info.plist local network keys) | ⬜ tested | configuration in place; **not built or run** — no Mac available in this environment |
| Real two-machine Mac ⇄ Windows testing | ⬜ | required before release |

## Not implemented (by design for this release)

* NAT traversal / relay for internet transfers — designed in ARCHITECTURE.md;
  direct connections via VPN or port forwarding work today.
* macOS Finder Share extension; macOS concealed-clipboard detection.
* Languages other than English.

## Verification log

* `cargo test -p droplink-core`: 20 unit + 25 integration tests pass on Windows 11.
* `cargo test --release -- --ignored`: 1 GiB transfer verified.
* `npm test` (6), `npm run typecheck`, `npm run build`: pass.
* `npm run tauri dev` on Windows 11: app launches, overview/sidebar/top bar render with live engine data;
  the headless `peer` example and the app discover each other over mDNS within seconds
  (peer listed as online, "Not paired", with address and port).
* Not yet done: clicking through pairing, send, receive and clipboard in the GUI against the peer
  (the same flows are covered by the engine integration tests).
