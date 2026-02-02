use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid hash format")]
    InvalidHash,
    #[error("invalid id map")]
    InvalidIdMap,
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
