//! DropLink core engine.
//!
//! Everything that touches the network, disk, clipboard or database lives in
//! this crate. It has no dependency on Tauri so the full engine can be driven
//! from integration tests (see `tests/`) with several instances running in one
//! process.

pub mod clipboard;
pub mod db;
pub mod discovery;
pub mod engine;
pub mod error;
pub mod events;
pub mod identity;
pub mod models;
pub mod net;
pub mod pairing;
pub mod protocol;
pub mod registry;
pub mod settings;
pub mod text;
pub mod tls;
pub mod transfer;
pub mod util;

pub use engine::{Engine, EngineConfig};
pub use error::{Error, Result};
pub use events::{Event, EventSink};
pub use settings::Settings;

/// Wire protocol version. Bumped on incompatible protocol changes.
pub const PROTOCOL_VERSION: u32 = 1;
/// mDNS service type advertised by every DropLink instance.
pub const SERVICE_TYPE: &str = "_droplink._tcp.local.";
/// Default TCP port for the secure transfer listener.
pub const DEFAULT_PORT: u16 = 47_100;
