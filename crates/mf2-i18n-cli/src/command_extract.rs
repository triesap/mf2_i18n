use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::artifacts::{write_catalog, write_id_map, write_id_map_hash};
use crate::config::load_config_or_default;
use crate::extract_pipeline::{ExtractPipelineError, extract_from_sources};

#[derive(Debug, Error)]
pub enum ExtractCommandError {
    #[error("config error: {0}")]
    Config(#[from] crate::error::CliError),
    #[error(transparent)]
    Pipeline(#[from] ExtractPipelineError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub project: String,
    pub roots: Vec<PathBuf>,
    pub out_dir: PathBuf,
    pub config_path: PathBuf,
    pub generated_at: String,
}

pub fn run_extract(options: &ExtractOptions) -> Result<(), ExtractCommandError> {
    let config = load_config_or_default(&options.config_path)?;
    let salt_path = resolve_path(&options.config_path, &config.project_salt_path);
    let salt = fs::read_to_string(&salt_path)?;
    let salt_bytes = salt.trim_end().as_bytes().to_vec();

    let output = extract_from_sources(
        &options.roots,
        &options.project,
        &config.default_locale,
        &options.generated_at,
        &salt_bytes,
    )?;

    fs::create_dir_all(&options.out_dir)?;
    write_catalog(&options.out_dir.join("i18n.catalog.json"), &output.catalog)?;
    write_id_map_hash(&options.out_dir.join("id_map_hash"), output.id_map_hash)?;
    write_id_map(&options.out_dir.join("id_map.json"), &output.id_map)?;
    Ok(())
}

fn resolve_path(config_path: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

#[cfg(test)]
mod tests {
    use super::{ExtractOptions, run_extract};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_extract_cmd_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn runs_extract_and_writes_outputs() {
        let dir = temp_dir();
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir).expect("src dir");
        fs::write(src_dir.join("lib.rs"), "let _ = t!(\"home.title\");").expect("src");

        let salt_path = dir.join("id_salt.txt");
        fs::write(&salt_path, "salt").expect("salt");

        let config_path = dir.join("mf2-i18n.toml");
        let config_contents = format!(
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nmicro_locales_registry = \"micro-locales.toml\"\nproject_salt_path = \"{}\"\n",
            salt_path.display()
        );
        fs::write(&config_path, config_contents).expect("config");

        let out_dir = dir.join("out");
        let options = ExtractOptions {
            project: "demo".to_string(),
            roots: vec![src_dir],
            out_dir: out_dir.clone(),
            config_path,
            generated_at: "2026-02-01T00:00:00Z".to_string(),
        };

        run_extract(&options).expect("run");
        assert!(out_dir.join("i18n.catalog.json").exists());
        assert!(out_dir.join("id_map_hash").exists());
        assert!(out_dir.join("id_map.json").exists());

        fs::remove_dir_all(&dir).ok();
    }
}
