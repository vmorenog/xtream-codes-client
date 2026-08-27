use std::fmt;

/// Every failure the frontend can be told about.
///
/// `Serialize` renders this as a plain string, so `invoke()` rejects with a
/// readable message instead of a tagged enum the UI would have to decode.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("no provider with id {0}")]
    UnknownProvider(i64),

    #[error("provider rejected the credentials")]
    BadCredentials,

    #[error("provider subscription is {0}")]
    NotEntitled(String),

    #[error("provider is unreachable: {0}")]
    Unreachable(String),

    #[error("provider sent something unexpected: {0}")]
    BadResponse(String),

    #[error("mpv was not found. Install it with `brew install mpv`.")]
    MpvMissing,

    #[error("mpv is not responding: {0}")]
    MpvUnresponsive(String),

    #[error("database error: {0}")]
    Db(String),

    #[error("keychain error: {0}")]
    Keychain(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        AppError::Keychain(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // reqwest's Display includes the request URL, which for us carries the
        // Provider password. Never let it reach a log or the UI.
        if e.is_decode() {
            AppError::BadResponse(redact(&e))
        } else {
            AppError::Unreachable(redact(&e))
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

/// Strip anything URL-shaped out of an error before it is shown or logged.
fn redact(e: &impl fmt::Display) -> String {
    let msg = e.to_string();
    msg.split_whitespace()
        .map(|w| {
            if w.contains("://") {
                "<url redacted>"
            } else {
                w
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
