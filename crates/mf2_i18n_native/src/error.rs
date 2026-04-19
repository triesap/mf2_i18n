use mf2_i18n_core::CoreError;
use mf2_i18n_runtime::RuntimeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NativeError {
    #[error("runtime not initialized")]
    NotInitialized,
    #[error("invalid utf-8 in embedded artifacts: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("runtime error: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("core error: {0}")]
    Core(String),
}

pub type NativeResult<T> = Result<T, NativeError>;

impl From<CoreError> for NativeError {
    fn from(value: CoreError) -> Self {
        Self::Core(value.to_string())
    }
}
