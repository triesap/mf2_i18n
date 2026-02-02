use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("core error: {0}")]
    Core(String),
    #[error("invalid hash format")]
    InvalidHash,
    #[error("invalid id map")]
    InvalidIdMap,
    #[error("hash mismatch for {0}")]
    HashMismatch(String),
    #[error("missing locale {0}")]
    MissingLocale(String),
    #[error("missing message key {0}")]
    MissingMessage(String),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

impl From<mf2_i18n_core::CoreError> for RuntimeError {
    fn from(err: mf2_i18n_core::CoreError) -> Self {
        RuntimeError::Core(err.to_string())
    }
}
