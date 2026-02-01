use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Clone, Deserialize)]
pub struct CliConfig {
    pub default_locale: String,
    pub source_dirs: Vec<String>,
    pub micro_locales_registry: Option<String>,
    pub project_salt_path: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            default_locale: "en".to_string(),
            source_dirs: vec!["locales".to_string()],
            micro_locales_registry: Some("micro-locales.toml".to_string()),
            project_salt_path: "tools/id_salt.txt".to_string(),
        }
    }
}

pub fn load_config(path: &Path) -> Result<CliConfig, CliError> {
    let contents = fs::read_to_string(path)?;
    let config = toml::from_str(&contents)?;
    Ok(config)
}

pub fn load_config_or_default(path: &Path) -> Result<CliConfig, CliError> {
    if path.exists() {
        load_config(path)
    } else {
        Ok(CliConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{load_config_or_default, CliConfig};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_{name}_{nanos}.toml"));
        path
    }

    #[test]
    fn uses_default_when_missing() {
        let path = temp_path("missing");
        let config = load_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "en");
    }

    #[test]
    fn loads_from_file() {
        let path = temp_path("config");
        let contents = r#"
default_locale = "fr"
source_dirs = ["locales"]
micro_locales_registry = "micro-locales.toml"
project_salt_path = "tools/id_salt.txt"
"#;
        fs::write(&path, contents).expect("write");
        let config = load_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "fr");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn default_values_are_stable() {
        let config = CliConfig::default();
        assert_eq!(config.project_salt_path, "tools/id_salt.txt");
    }
}
