use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
pub enum SakiError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Connection timeout")]
    Timeout,
    #[error("Query cancelled")]
    Cancelled,
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Not connected")]
    NotConnected,
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),
    #[error("Not supported: {0}")]
    NotSupported(String),
}

pub type Result<T> = std::result::Result<T, SakiError>;
