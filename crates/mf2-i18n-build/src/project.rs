use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::config::{ProjectConfig, load_project_config, load_project_config_or_default};
use crate::error::BuildIoError;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error(transparent)]
    Config(#[from] BuildIoError),
    #[error("project salt must not be empty")]
    EmptySalt,
}

#[derive(Debug, Clone)]
pub struct ProjectLayout {
    config_path: PathBuf,
    config: ProjectConfig,
}

impl ProjectLayout {
    pub fn load(config_path: &Path) -> Result<Self, ProjectError> {
        Ok(Self {
            config_path: config_path.to_path_buf(),
            config: load_project_config(config_path)?,
        })
    }

    pub fn load_or_default(config_path: &Path) -> Result<Self, ProjectError> {
        Ok(Self {
            config_path: config_path.to_path_buf(),
            config: load_project_config_or_default(config_path)?,
        })
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn base_dir(&self) -> &Path {
        self.config_path.parent().unwrap_or_else(|| Path::new("."))
    }

    pub fn resolve_path(&self, value: &str) -> PathBuf {
        resolve_config_relative_path(&self.config_path, value)
    }

    pub fn source_roots(&self) -> Vec<PathBuf> {
        self.config
            .source_dirs
            .iter()
            .map(|value| self.resolve_path(value))
            .collect()
    }

    pub fn micro_locales_registry_path(&self) -> Option<PathBuf> {
        self.config
            .micro_locales_registry
            .as_deref()
            .map(|value| self.resolve_path(value))
    }

    pub fn project_salt_path(&self) -> PathBuf {
        self.resolve_path(&self.config.project_salt_path)
    }

    pub fn load_project_salt(&self) -> Result<Vec<u8>, ProjectError> {
        let salt = fs::read_to_string(self.project_salt_path()).map_err(BuildIoError::from)?;
        let salt = salt.trim_end().as_bytes().to_vec();
        if salt.is_empty() {
            return Err(ProjectError::EmptySalt);
        }
        Ok(salt)
    }
}

pub fn resolve_config_relative_path(config_path: &Path, value: &str) -> PathBuf {
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
    use super::ProjectLayout;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_project_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn resolves_project_relative_paths_and_salt() {
        let root = temp_dir();
        let config_path = root.join("mf2-i18n.toml");
        let salt_dir = root.join("tools");
        fs::create_dir_all(&salt_dir).expect("salt dir");
        fs::write(salt_dir.join("id_salt.txt"), "salt").expect("salt");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\", \"shared/locales\"]\nmicro_locales_registry = \"micro-locales.toml\"\nproject_salt_path = \"tools/id_salt.txt\"\n",
        )
        .expect("config");

        let layout = ProjectLayout::load(&config_path).expect("layout");
        assert_eq!(layout.source_roots().len(), 2);
        assert_eq!(
            layout
                .micro_locales_registry_path()
                .expect("micro locale path")
                .file_name()
                .and_then(|name| name.to_str()),
            Some("micro-locales.toml")
        );
        assert_eq!(layout.load_project_salt().expect("salt"), b"salt");

        fs::remove_dir_all(&root).ok();
    }
}
