use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
}
