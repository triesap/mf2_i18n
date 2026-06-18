use std::fs;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum InitCommandError {
    #[error("config already exists: {0}")]
    ConfigExists(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub config_path: PathBuf,
    pub preset: String,
    pub source_layout: String,
    pub target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub config_path: PathBuf,
}

pub fn run_init(options: &InitOptions) -> Result<InitReport, InitCommandError> {
    if options.config_path.exists() {
        return Err(InitCommandError::ConfigExists(
            options.config_path.display().to_string(),
        ));
    }
    if let Some(parent) = options.config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&options.config_path, render_config(options))?;
    Ok(InitReport {
        config_path: options.config_path.clone(),
    })
}

fn render_config(options: &InitOptions) -> String {
    format!(
        "version = 1\n\
default_locale = \"en\"\n\
locales = [\"en\"]\n\n\
[[sources]]\n\
name = \"site\"\n\
layout = {source_layout:?}\n\
root = \"locales\"\n\
namespace_glob = \"*.mf2\"\n\n\
[[targets]]\n\
name = {target_name:?}\n\
source = \"site\"\n\
preset = {preset:?}\n\
out_dir = \"src/generated/mf2\"\n\
typescript = true\n",
        source_layout = options.source_layout,
        target_name = options.target_name,
        preset = options.preset
    )
}

#[cfg(test)]
mod tests {
    use super::{InitCommandError, InitOptions, run_init};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_init_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn writes_v1_config() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path: config_path.clone(),
            preset: "vite".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "web".to_string(),
        })
        .expect("init");

        let contents = fs::read_to_string(config_path).expect("read");
        assert!(contents.contains("version = 1"));
        assert!(contents.contains("preset = \"vite\""));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_existing_config() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(&config_path, "").expect("write");

        let err = run_init(&InitOptions {
            config_path,
            preset: "vite".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "web".to_string(),
        })
        .expect_err("exists");

        assert!(matches!(err, InitCommandError::ConfigExists(_)));

        fs::remove_dir_all(&dir).ok();
    }
}
