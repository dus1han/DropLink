-- DropLink schema v1.
-- Device IDs are lowercase hex SHA-256 fingerprints of each device's
-- certificate. Timestamps are Unix milliseconds.

CREATE TABLE devices (
    id               TEXT PRIMARY KEY CHECK (length(id) = 64),
    name             TEXT NOT NULL,              -- name the device advertises
    alias            TEXT,                       -- local rename, if any
    os               TEXT NOT NULL DEFAULT 'unknown',
    app_version      TEXT,
    last_address     TEXT,
    last_port        INTEGER,
    connection_type  TEXT NOT NULL DEFAULT 'lan' CHECK (connection_type IN ('lan', 'manual')),
    manual_address   TEXT,                       -- host:port typed by the user
    first_seen       INTEGER NOT NULL,
    last_seen        INTEGER NOT NULL
);
CREATE INDEX idx_devices_last_seen ON devices (last_seen DESC);

CREATE TABLE trusted_devices (
    device_id        TEXT PRIMARY KEY REFERENCES devices (id) ON DELETE CASCADE,
    status           TEXT NOT NULL CHECK (status IN ('trusted', 'blocked')),
    paired_at        INTEGER,
    auto_accept      INTEGER NOT NULL DEFAULT 0 CHECK (auto_accept IN (0, 1)),
    clipboard_sync   INTEGER NOT NULL DEFAULT 1 CHECK (clipboard_sync IN (0, 1)),
    updated_at       INTEGER NOT NULL
);
CREATE INDEX idx_trusted_status ON trusted_devices (status);

CREATE TABLE device_groups (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(trim(name)) > 0),
    color            TEXT NOT NULL DEFAULT 'blue',
    auto_accept      INTEGER NOT NULL DEFAULT 0 CHECK (auto_accept IN (0, 1)),
    clipboard_sync   INTEGER NOT NULL DEFAULT 0 CHECK (clipboard_sync IN (0, 1)),
    created_at       INTEGER NOT NULL
);

CREATE TABLE device_group_members (
    group_id         TEXT NOT NULL REFERENCES device_groups (id) ON DELETE CASCADE,
    device_id        TEXT NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    added_at         INTEGER NOT NULL,
    PRIMARY KEY (group_id, device_id)
);
CREATE INDEX idx_group_members_device ON device_group_members (device_id);

-- One send action (outgoing) or one received offer (incoming).
CREATE TABLE transfer_sessions (
    id               TEXT PRIMARY KEY,
    direction        TEXT NOT NULL CHECK (direction IN ('outgoing', 'incoming')),
    sender_device_id TEXT NOT NULL,
    label            TEXT NOT NULL,
    total_bytes      INTEGER NOT NULL DEFAULT 0 CHECK (total_bytes >= 0),
    total_files      INTEGER NOT NULL DEFAULT 0 CHECK (total_files >= 0),
    dest_dir         TEXT,                       -- incoming only
    conflict_policy  TEXT,                       -- incoming only
    created_at       INTEGER NOT NULL
);
CREATE INDEX idx_sessions_created ON transfer_sessions (created_at DESC);

CREATE TABLE transfer_items (
    session_id       TEXT NOT NULL REFERENCES transfer_sessions (id) ON DELETE CASCADE,
    idx              INTEGER NOT NULL CHECK (idx >= 0),
    rel_path         TEXT NOT NULL,
    source_path      TEXT,                       -- outgoing: absolute path on disk
    final_path       TEXT,                       -- incoming: resolved destination
    size             INTEGER NOT NULL CHECK (size >= 0),
    mtime            INTEGER,
    is_dir           INTEGER NOT NULL DEFAULT 0 CHECK (is_dir IN (0, 1)),
    sha256           TEXT,
    status           TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed')),
    PRIMARY KEY (session_id, idx)
);

-- One delivery per (session, device). For outgoing sessions this is one row
-- per recipient; its id is the session id used on the wire. Incoming
-- sessions have exactly one row whose id equals the session id.
CREATE TABLE transfer_recipients (
    id               TEXT PRIMARY KEY,
    session_id       TEXT NOT NULL REFERENCES transfer_sessions (id) ON DELETE CASCADE,
    device_id        TEXT NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('queued', 'connecting', 'waiting', 'transferring',
                                                     'paused', 'completed', 'failed', 'cancelled', 'rejected')),
    bytes_done       INTEGER NOT NULL DEFAULT 0,
    files_done       INTEGER NOT NULL DEFAULT 0,
    error            TEXT,
    verified         INTEGER CHECK (verified IN (0, 1)),
    auto_resume      INTEGER NOT NULL DEFAULT 0 CHECK (auto_resume IN (0, 1)),
    started_at       INTEGER,
    finished_at      INTEGER,
    updated_at       INTEGER NOT NULL
);
CREATE INDEX idx_recipients_session ON transfer_recipients (session_id);
CREATE INDEX idx_recipients_status ON transfer_recipients (status);
CREATE INDEX idx_recipients_device ON transfer_recipients (device_id);

-- Denormalised record of every finished delivery. Names are copied so history
-- stays readable after a device is forgotten.
CREATE TABLE transfer_history (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    delivery_id      TEXT NOT NULL UNIQUE,
    direction        TEXT NOT NULL CHECK (direction IN ('outgoing', 'incoming')),
    kind             TEXT NOT NULL CHECK (kind IN ('files', 'folder', 'text')),
    name             TEXT NOT NULL,
    total_bytes      INTEGER NOT NULL DEFAULT 0,
    file_count       INTEGER NOT NULL DEFAULT 0,
    sender_id        TEXT NOT NULL,
    sender_name      TEXT NOT NULL,
    recipient_id     TEXT NOT NULL,
    recipient_name   TEXT NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'cancelled', 'rejected')),
    error            TEXT,
    verified         INTEGER CHECK (verified IN (0, 1)),
    started_at       INTEGER,
    finished_at      INTEGER NOT NULL,
    duration_ms      INTEGER
);
CREATE INDEX idx_history_finished ON transfer_history (finished_at DESC);
CREATE INDEX idx_history_status ON transfer_history (status);
CREATE INDEX idx_history_sender ON transfer_history (sender_id);
CREATE INDEX idx_history_recipient ON transfer_history (recipient_id);

CREATE TABLE text_messages (
    id               TEXT PRIMARY KEY,
    direction        TEXT NOT NULL CHECK (direction IN ('outgoing', 'incoming')),
    device_id        TEXT NOT NULL,
    device_name      TEXT NOT NULL,
    content          TEXT NOT NULL,
    created_at       INTEGER NOT NULL
);
CREATE INDEX idx_text_created ON text_messages (created_at DESC);

-- Image clipboard entries keep only metadata; pixel data is never stored.
CREATE TABLE clipboard_history (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    origin           TEXT NOT NULL CHECK (origin IN ('local', 'remote')),
    device_id        TEXT,
    device_name      TEXT,
    kind             TEXT NOT NULL CHECK (kind IN ('text', 'image')),
    content          TEXT,
    width            INTEGER,
    height           INTEGER,
    byte_size        INTEGER NOT NULL DEFAULT 0,
    hash             TEXT NOT NULL,
    created_at       INTEGER NOT NULL
);
CREATE INDEX idx_clipboard_created ON clipboard_history (created_at DESC);

CREATE TABLE application_settings (
    key              TEXT PRIMARY KEY,
    value            TEXT NOT NULL,              -- JSON
    updated_at       INTEGER NOT NULL
);

CREATE TABLE security_events (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    kind             TEXT NOT NULL,
    device_id        TEXT,
    device_name      TEXT,
    detail           TEXT NOT NULL,
    created_at       INTEGER NOT NULL
);
CREATE INDEX idx_security_created ON security_events (created_at DESC);
