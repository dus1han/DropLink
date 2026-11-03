# DropLink architecture

DropLink is a Tauri 2 desktop app. All networking, storage, cryptography and
file I/O is in Rust; the React frontend only renders state and calls typed
commands.

```
┌──────────────────────────── Desktop app (src-tauri) ────────────────────────────┐
│  React UI (src/)  ──invoke──▶  commands.rs  ──▶  droplink-core::Engine          │
│        ▲                                             │                          │
│        └──── "droplink://event" ◀── bridge.rs ◀── EventSink (+ native notices)  │
│  tray · single-instance · autostart · dialogs · "Send to" (platform.rs)         │
└─────────────────────────────────────────────────────────────────────────────────┘
                     │ TLS 1.3, mutual auth, framed protocol (TCP 47100)
                     ▼
              other DropLink devices  ◀── mDNS _droplink._tcp ──▶  discovery
```

## Crates and folders

| Path | Purpose |
| --- | --- |
| `crates/droplink-core` | The engine. No Tauri dependency, fully testable in-process. |
| `src-tauri` | Tauri shell: commands, event bridge, notifications, tray, OS integration, packaging. |
| `src` | React + TypeScript UI (Vite, Tailwind CSS v4, Framer Motion, Lucide, Zustand). |

### `droplink-core` modules

| Module | Responsibility |
| --- | --- |
| `engine.rs` | `Engine` handle (cheap `Arc` clone): start-up, device views, groups, settings, stats, security log. |
| `identity.rs` | Per-device ECDSA P-256 key + self-signed certificate. Device ID = SHA-256 of the certificate. |
| `tls.rs` | rustls TLS 1.3 server/client configs with mutual authentication (see SECURITY.md). |
| `protocol.rs` | Wire format: `[kind u8][len u32][payload]`; JSON control messages, raw data frames. |
| `net/` | Connect / accept, `Hello` exchange binding the claimed ID to the certificate, dispatch by purpose, graceful close. |
| `discovery.rs` | mDNS advertise + browse (`mdns-sd`), feeding the registry. |
| `registry.rs` | In-memory presence (online, addresses, port) keyed by device ID — one entry per device regardless of how it was found. |
| `pairing.rs` | Commit–reveal short-authentication-string pairing. |
| `transfer/` | Delivery tasks, outgoing streamer, incoming receiver, concurrency gate, rate limiter, progress, resume. |
| `text.rs` | Instant text messages. |
| `clipboard/` | Watcher, loop prevention, sync, platform backends (`system.rs`). |
| `db/` | SQLite repository + migrations. |
| `events.rs` / `models.rs` | Everything the UI sees (serialised camelCase, mirrored in `src/lib/types.ts`). |

## Connections

Every connection is short-lived and has one **purpose**, declared in the first
message:

1. TCP connect → TLS 1.3 handshake with client certificates.
2. Client sends `Hello { device_id, name, os, port, purpose }`. The server
   computes the SHA-256 of the client's certificate and refuses the
   connection unless it equals `device_id`. It answers with its own `Hello`,
   which the client checks the same way (and against the ID it expected).
3. Blocked devices get `Error{blocked}`. Other purposes check trust on their own:
   `Probe` and `Pair` are open to unknown devices; `Transfer`, `Text`,
   `Clipboard` and `Control` require a trusted device.
4. The server closes gracefully (close_notify + drain) so error messages are
   never lost to a TCP reset (a real issue on Windows).

Using a connection per operation keeps the protocol simple, makes concurrent
transfers independent, and means no long-lived socket has to be kept alive
across sleep/network changes.

Axum/WebSockets were considered and rejected for peer traffic: an HTTP layer
adds nothing when both ends are DropLink, while a custom framed protocol over
TLS gives zero-copy-ish raw data frames and exact control over resume and
backpressure.

## Discovery and presence

* Each instance advertises `_droplink._tcp.local.` with TXT `id`, `name`, `os`, `pv`.
* Browse results update the `devices` table (persistent) and the `Registry`
  (presence). Removal events and failed connections mark a device offline.
* Any authenticated inbound connection also marks the peer online with its
  address — so devices reached through VPNs or port forwarding show up too.
* TXT claims are display-only; identity is proven at connection time.

## Transfers

* A **session** is one send action; a **delivery** is that session to one
  device (`transfer_recipients`). Each delivery is its own Tokio task with its
  own progress, pause, cancel and retry, so a slow or failing recipient never
  affects the others.
* `Gate` limits simultaneous deliveries (configurable); `RateLimiter` is a
  shared token bucket for the upload limit.
* Flow: `Offer{session_id, items}` → receiver validates paths, applies approval
  (prompt / auto-accept per device or group) → `Accept{offsets}` →
  `FileStart` / data frames (256 KiB) / `FileEnd{sha256}` … → `Complete` →
  `Summary{failed}`.
* Files are read and written in chunks — memory use is constant regardless of
  file size (verified with a 1 GiB test).
* The receiver writes to `<dest>/.droplink-partial/<delivery>/<n>.part` and
  only moves a file into place after its SHA-256 matches. Partial files are
  what make **pause, network interruption and app restarts** resumable: the
  next attempt reuses the same session id and the receiver answers with how
  many bytes it already has. Both sides re-hash the existing prefix locally so
  the final digest still covers the whole file.
* Interruptions: the sender retries with back-off (2 s, 5 s, 12 s), then parks
  the delivery as *paused* and resumes automatically when the device is seen
  again (mDNS or inbound connection). Deliveries running when the app quit are
  restored as paused at the next start.
* Name conflicts are resolved once per top-level item at accept time ("Keep
  both" → `name (2)`, or "Replace"), so a renamed folder stays consistent
  across resumes.

## Clipboard

The watcher polls a cheap change indicator every 700 ms only while automatic
sync is on (Windows: `GetClipboardSequenceNumber`; macOS/Linux: content hash).
Loop prevention is hash-based: applying remote content records its hash and
the post-write change counter as already seen, so it is never re-broadcast.
See `clipboard/mod.rs` and SECURITY.md for the trust rules.

## Data model (SQLite, `db/migrations/001_initial.sql`)

`devices`, `trusted_devices`, `device_groups`, `device_group_members`,
`transfer_sessions`, `transfer_items`, `transfer_recipients`,
`transfer_history`, `text_messages`, `clipboard_history`,
`application_settings`, `security_events` — with foreign keys (cascading),
CHECK constraints on enums, and indexes on every list/filter column.
Migrations are applied in order using `PRAGMA user_version`.

## Frontend

* `src/store/app.ts` — single Zustand store, initialised from `bootstrap` and
  kept current by engine events (no polling).
* `src/lib/api.ts` — one typed wrapper per Rust command.
* `src/components/ui` — design-system primitives; all colours come from CSS
  tokens in `src/styles/index.css` (dark and light themes).
* `src/pages` — one file per screen.

## Internet transfers (current scope)

Implemented: direct connections to any reachable address (VPN such as
Tailscale/ZeroTier, port forwarding), with the same pairing and encryption,
plus a setting that refuses non-private source addresses unless enabled.

Designed but **not implemented**: a signaling service for NAT traversal and an
optional relay. The intended design keeps the trust model unchanged — the
signaling server would only exchange candidate addresses between devices that
already know each other's certificate fingerprints; a relay would forward
opaque TLS bytes (end-to-end encryption between the devices, so the relay
cannot read or alter content), and the UI would label relay-assisted
deliveries distinctly from direct ones. Nothing in the current build talks to
any server.
