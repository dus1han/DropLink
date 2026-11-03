use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),
    #[error("invalid message: {0}")]
    Json(#[from] serde_json::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("this device is not trusted — pair it first")]
    NotTrusted,
    #[error("the device is blocked")]
    Blocked,
    #[error("device identity does not match the paired certificate")]
    IdentityMismatch,
    #[error("unknown device")]
    DeviceNotFound,
    #[error("the device is not reachable")]
    Unreachable,
    #[error("{0}")]
    Rejected(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("timed out")]
    Timeout,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Errors caused by the network (as opposed to a decision by either user)
    /// are worth retrying automatically.
    pub fn is_transient(&self) -> bool {
        matches!(self, Error::Io(_) | Error::Tls(_) | Error::Timeout | Error::Unreachable)
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Error::Timeout
    }
}

impl From<rcgen::Error> for Error {
    fn from(e: rcgen::Error) -> Self {
        Error::Other(format!("certificate generation failed: {e}"))
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
