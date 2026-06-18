use std::path::PathBuf;

use mf2_i18n_build::ProjectLayout;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScaffoldCommandError {
    #[error(transparent)]
    Project(#[from] mf2_i18n_build::ProjectError),
    #[error("source {0} does not exist")]
    UnknownSource(String),
    #[error("locale must not be empty")]
    EmptyLocale,
}

#[derive(Debug, Clone)]
pub struct ScaffoldMissingOptions {
    pub config_path: PathBuf,
    pub source_name: String,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldMissingReport {
    pub source_name: String,
    pub locale: String,
}

pub fn run_scaffold_missing(
    options: &ScaffoldMissingOptions,
) -> Result<ScaffoldMissingReport, ScaffoldCommandError> {
    if options.locale.trim().is_empty() {
        return Err(ScaffoldCommandError::EmptyLocale);
    }
    let project = ProjectLayout::load(&options.config_path)?;
    if !project
        .config()
        .sources
        .iter()
        .any(|source| source.name == options.source_name)
    {
        return Err(ScaffoldCommandError::UnknownSource(
            options.source_name.clone(),
        ));
    }
    Ok(ScaffoldMissingReport {
        source_name: options.source_name.clone(),
        locale: options.locale.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{ScaffoldMissingOptions, run_scaffold_missing};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_scaffold_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn validates_requested_source_and_locale() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
"#,
        )
        .expect("config");

        let report = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path,
            source_name: "site".to_string(),
            locale: "es".to_string(),
        })
        .expect("scaffold");

        assert_eq!(report.source_name, "site");
        assert_eq!(report.locale, "es");

        fs::remove_dir_all(&dir).ok();
    }
}
