use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Unsupported(&'static str),
    InvalidInput(&'static str),
    Internal(&'static str),
}

pub type CoreResult<T> = Result<T, CoreError>;

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Unsupported(message) => write!(f, "unsupported: {message}"),
            CoreError::InvalidInput(message) => write!(f, "invalid input: {message}"),
            CoreError::Internal(message) => write!(f, "internal error: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CoreError {}

#[cfg(test)]
mod tests {
    use super::CoreError;
    use alloc::string::ToString;

    #[test]
    fn display_formats_unsupported() {
        let err = CoreError::Unsupported("feature");
        assert_eq!(err.to_string(), "unsupported: feature");
    }

    #[test]
    fn display_formats_invalid_input() {
        let err = CoreError::InvalidInput("arg");
        assert_eq!(err.to_string(), "invalid input: arg");
    }

    #[test]
    fn display_formats_internal() {
        let err = CoreError::Internal("state");
        assert_eq!(err.to_string(), "internal error: state");
    }
}
