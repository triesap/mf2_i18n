use std::fs;
use std::path::PathBuf;

use mfs_i18n_build::BuildIoError;
use mfs_i18n_build::artifacts::{write_catalog, write_id_map, write_id_map_hash};
use mfs_i18n_build::extract_pipeline::{ExtractPipelineError, extract_from_sources};
use mfs_i18n_build::project::{ProjectError, ProjectLayout};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractCommandError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Pipeline(#[from] ExtractPipelineError),
    #[error(transparent)]
    BuildIo(#[from] BuildIoError),
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
    let project = ProjectLayout::load_or_default(&options.config_path)?;
    let salt_bytes = project.load_project_salt()?;

    let output = extract_from_sources(
        &options.roots,
        &options.project,
        &project.config().default_locale,
        &options.generated_at,
        &salt_bytes,
    )?;

    fs::create_dir_all(&options.out_dir)?;
    write_catalog(&options.out_dir.join("i18n.catalog.json"), &output.catalog)?;
    write_id_map_hash(&options.out_dir.join("id_map_hash"), output.id_map_hash)?;
    write_id_map(&options.out_dir.join("id_map.json"), &output.id_map)?;
    Ok(())
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
        path.push(format!("mfs_i18n_extract_cmd_{nanos}"));
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

        let config_path = dir.join("mfs_i18n.toml");
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
