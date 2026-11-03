# DropLink security model

## Goals

* Only devices the user has explicitly paired can send files, text or
  clipboard content.
* Everything on the wire is encrypted and integrity-protected.
* A device cannot impersonate another, even on the same network.
* Received data is verified before it is kept, and can never be written
  outside the chosen destination folder.
* No cloud storage, relay or third-party server is involved.

## Device identity

* On first launch each device generates an **ECDSA P-256** key pair and a
  self-signed X.509 certificate (`rcgen`), stored in the app data folder
  (`identity.key` with mode 0600 on Unix; per-user profile ACLs on Windows).
* The **device ID is the SHA-256 fingerprint of the certificate**. It is shown
  (abbreviated) in Settings → Security and Trusted Devices.

## Transport

* TLS 1.3 only (rustls with the `ring` provider). TLS 1.2 and below are
  refused.
* Mutual authentication: both sides present certificates. Since certificates
  are self-signed, there is no CA chain to validate; instead the verifiers
  **still verify the handshake signature**, which proves possession of the
  private key, and the engine derives the peer's ID from the certificate.
* The first application message (`Hello`) must claim exactly that ID or the
  connection is dropped and a security event is recorded
  (`identity_mismatch`). When connecting to a known device, the certificate
  must also match the device we meant to reach.
* Covered by tests: `certificate_must_match_expected_identity`,
  `claiming_a_trusted_id_without_its_key_is_rejected`.

## Pairing

Short-authentication-string (SAS) protocol over the authenticated TLS channel:

1. Initiator sends `commit = SHA-256(nonce_i)`.
2. Responder replies with `nonce_r`.
3. Initiator reveals `nonce_i`; responder checks the commitment.
4. Both compute `code = SHA-256("droplink-pair-v1" ‖ id_initiator ‖ id_responder ‖ nonce_i ‖ nonce_r) mod 10⁶`.
5. Both users compare the 6-digit code; trust is stored only if **both**
   confirm.

A man-in-the-middle would hold two TLS sessions with different certificates,
so the codes differ. The commitment prevents him from picking his nonce after
seeing the initiator's, so he cannot search for a colliding code; his chance
of success is 1 in 1,000,000 per attempt, and each attempt is visible to both
users. Unknown devices are **never** trusted automatically. Pairing requests
are limited to 4 concurrent.

## Authorisation

| Action | Requirement |
| --- | --- |
| Discover / probe / pair | any device that is not blocked |
| Send files, text | sender is trusted by the receiver |
| Clipboard | trusted **and** clipboard permission (device or group) **and** (manual send or receiver has automatic sync on) |
| Skip approval prompt | device or one of its groups has auto-accept, or the user disabled approvals globally |
| Blocked device | refused at connection, before any purpose is handled |

Revoking trust or blocking cancels that device's running transfers. Group
permissions never grant anything to untrusted or blocked members (enforced in
SQL, see `effective_auto_accept`).

## Incoming data validation

* Paths in offers are `/`-separated relative paths; `..`, empty components,
  absolute paths, NUL bytes are rejected; Windows-reserved names and
  characters are neutralised (`util::safe_relative_path`). Tested by
  `path_traversal_offers_are_rejected`.
* Session IDs must be UUIDs (they become folder names); a session ID belonging
  to another device is refused (`session_hijack`).
* Frame sizes are bounded (32 MiB control, 64 MiB data); offers are limited to
  200,000 items; the receiver refuses more bytes than a file announced.
* Every file is hashed with SHA-256 while streaming on both sides. The
  receiver moves a file into place only if the digests match; otherwise the
  data is deleted, the item is marked failed and a security event is recorded
  (`corrupted_data_fails_sha256_verification`).
* Symbolic links are never followed when sending folders.

## Network exposure

* The listener accepts connections only from private, link-local, loopback
  and CGNAT (100.64/10, used by Tailscale) addresses unless *Accept
  connections from other networks* is enabled.
* Windows installers add a firewall rule for **private and domain** profiles
  only — never public networks.

## Clipboard privacy

* Automatic sync is **off by default**.
* On Windows, content carrying the formats password managers use to mark
  secrets (`ExcludeClipboardContentFromMonitorProcessing`,
  `CanIncludeInClipboardHistory`, `Clipboard Viewer Ignore`) is never synced
  or recorded.
* Image history stores only dimensions, never pixels. Text history is capped
  (20,000 characters per item, configurable item count) and is deleted when
  history is turned off.
* Limitation: macOS `org.nspasteboard.ConcealedType` is not visible through
  the clipboard library currently used; see PLATFORM_NOTES.md.

## What is stored

App data folder: identity key and certificate, `droplink.db` (devices, trust,
groups, transfer metadata and history, text messages you sent/received,
clipboard history, settings, security log), and logs. File contents are only
ever written to the destination folder the user chose.

## Reporting

Security events are visible in Trusted Devices → Security log and are
retained (bounded to the last 2,000).
