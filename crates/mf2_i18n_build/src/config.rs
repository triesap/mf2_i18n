use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::BuildIoError;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub default_locale: String,
    pub source_dirs: Vec<String>,
    pub micro_locales_registry: Option<String>,
    pub project_salt_path: String,
    #[serde(default)]
    pub web: WebConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebConfig {
    #[serde(default)]
    pub targets: Vec<WebTargetConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebTargetConfig {
    pub name: String,
    pub mode: WebTargetMode,
    #[serde(default)]
    pub framework: Option<WebFramework>,
    pub out_dir: String,
    #[serde(default)]
    pub asset_dir: Option<String>,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub namespaces: Vec<String>,
    #[serde(default)]
    pub docs_source_dir: Option<String>,
    #[serde(default)]
    pub docusaurus_i18n_dir: Option<String>,
    #[serde(default)]
    pub docusaurus_sidebar_id: Option<String>,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub release_id: Option<String>,
    #[serde(default)]
    pub stale_policy: Option<WebStalePolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebTargetMode {
    PlainJson,
    Runtime,
    Docusaurus,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebFramework {
    Vanilla,
    React,
    Svelte,
    Vite,
    Docusaurus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebStalePolicy {
    Ignore,
    Delete,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WebTargetConfigError {
    #[error("web target name must not be empty")]
    EmptyTargetName,
    #[error("duplicate web target name {0}")]
    DuplicateTargetName(String),
    #[error("web target {target} out_dir must not be empty")]
    EmptyOutDir { target: String },
    #[error("web target {target} asset_dir must not be empty when provided")]
    EmptyAssetDir { target: String },
    #[error("web target {target} locale entries must not be empty")]
    EmptyLocale { target: String },
    #[error("web target {target} namespace entries must not be empty")]
    EmptyNamespace { target: String },
    #[error("web target {target} hybrid mode does not support docusaurus framework")]
    HybridDocusaurusFramework { target: String },
    #[error("web target {target} docusaurus mode requires docs_source_dir")]
    MissingDocusaurusDocsSourceDir { target: String },
    #[error("web target {target} docusaurus mode requires docusaurus_i18n_dir")]
    MissingDocusaurusI18nDir { target: String },
    #[error("web target {target} docusaurus_sidebar_id must not be empty when provided")]
    EmptyDocusaurusSidebarId { target: String },
    #[error(
        "web target {target} docusaurus mode must use docusaurus framework when framework is set"
    )]
    DocusaurusFrameworkMismatch { target: String },
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            default_locale: "en".to_string(),
            source_dirs: vec!["locales".to_string()],
            micro_locales_registry: Some("micro-locales.toml".to_string()),
            project_salt_path: "id_salt.txt".to_string(),
            web: WebConfig::default(),
        }
    }
}

impl ProjectConfig {
    pub fn validate_web_targets(&self) -> Result<(), WebTargetConfigError> {
        self.web.validate()
    }
}

impl WebConfig {
    pub fn target(&self, name: &str) -> Option<&WebTargetConfig> {
        self.targets.iter().find(|target| target.name == name)
    }

    pub fn validate(&self) -> Result<(), WebTargetConfigError> {
        let mut names = std::collections::BTreeSet::new();
        for target in &self.targets {
            target.validate()?;
            if !names.insert(target.name.clone()) {
                return Err(WebTargetConfigError::DuplicateTargetName(
                    target.name.clone(),
                ));
            }
        }
        Ok(())
    }
}

impl WebTargetConfig {
    pub fn validate(&self) -> Result<(), WebTargetConfigError> {
        if self.name.trim().is_empty() {
            return Err(WebTargetConfigError::EmptyTargetName);
        }

        if self.out_dir.trim().is_empty() {
            return Err(WebTargetConfigError::EmptyOutDir {
                target: self.name.clone(),
            });
        }

        if self
            .asset_dir
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(WebTargetConfigError::EmptyAssetDir {
                target: self.name.clone(),
            });
        }

        if self.locales.iter().any(|locale| locale.trim().is_empty()) {
            return Err(WebTargetConfigError::EmptyLocale {
                target: self.name.clone(),
            });
        }

        if self
            .namespaces
            .iter()
            .any(|namespace| namespace.trim().is_empty())
        {
            return Err(WebTargetConfigError::EmptyNamespace {
                target: self.name.clone(),
            });
        }

        match self.mode {
            WebTargetMode::PlainJson | WebTargetMode::Runtime => Ok(()),
            WebTargetMode::Hybrid => self.validate_hybrid(),
            WebTargetMode::Docusaurus => self.validate_docusaurus(),
        }
    }

    fn validate_hybrid(&self) -> Result<(), WebTargetConfigError> {
        if self
            .framework
            .is_some_and(|framework| matches!(framework, WebFramework::Docusaurus))
        {
            return Err(WebTargetConfigError::HybridDocusaurusFramework {
                target: self.name.clone(),
            });
        }

        Ok(())
    }

    fn validate_docusaurus(&self) -> Result<(), WebTargetConfigError> {
        if self
            .docs_source_dir
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(WebTargetConfigError::MissingDocusaurusDocsSourceDir {
                target: self.name.clone(),
            });
        }

        if self
            .docusaurus_i18n_dir
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(WebTargetConfigError::MissingDocusaurusI18nDir {
                target: self.name.clone(),
            });
        }

        if self
            .docusaurus_sidebar_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(WebTargetConfigError::EmptyDocusaurusSidebarId {
                target: self.name.clone(),
            });
        }

        if self
            .framework
            .is_some_and(|framework| !matches!(framework, WebFramework::Docusaurus))
        {
            return Err(WebTargetConfigError::DocusaurusFrameworkMismatch {
                target: self.name.clone(),
            });
        }

        Ok(())
    }
}

pub fn load_project_config(path: &Path) -> Result<ProjectConfig, BuildIoError> {
    let contents = fs::read_to_string(path)?;
    let config = toml::from_str(&contents)?;
    Ok(config)
}

pub fn load_project_config_or_default(path: &Path) -> Result<ProjectConfig, BuildIoError> {
    if path.exists() {
        load_project_config(path)
    } else {
        Ok(ProjectConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectConfig, WebFramework, WebStalePolicy, WebTargetConfigError, WebTargetMode,
        load_project_config_or_default,
    };
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
        let config = load_project_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "en");
    }

    #[test]
    fn loads_from_file() {
        let path = temp_path("config");
        let contents = r#"
default_locale = "fr"
source_dirs = ["locales"]
micro_locales_registry = "micro-locales.toml"
project_salt_path = "id_salt.txt"
"#;
        fs::write(&path, contents).expect("write");
        let config = load_project_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "fr");
        assert!(config.web.targets.is_empty());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn default_values_are_stable() {
        let config = ProjectConfig::default();
        assert_eq!(config.project_salt_path, "id_salt.txt");
        assert!(config.web.targets.is_empty());
        assert!(config.validate_web_targets().is_ok());
    }

    #[test]
    fn loads_web_targets() {
        let path = temp_path("web");
        fs::write(
            &path,
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "docusaurus"
framework = "docusaurus"
out_dir = "src/generated/mf2"
asset_dir = "static/mf2"
locales = ["en", "es"]
namespaces = ["home", "docs"]
docs_source_dir = "docs"
docusaurus_i18n_dir = "i18n"
generated_at = "2026-06-17T00:00:00Z"
release_id = "docs-local"
stale_policy = "fail"
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        let target = config.web.target("docs").expect("target");
        assert_eq!(target.mode, WebTargetMode::Docusaurus);
        assert_eq!(target.framework, Some(WebFramework::Docusaurus));
        assert_eq!(target.stale_policy, Some(WebStalePolicy::Fail));
        assert_eq!(target.locales, ["en".to_string(), "es".to_string()]);
        assert!(config.validate_web_targets().is_ok());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_duplicate_web_target_names() {
        let path = temp_path("web_duplicate");
        fs::write(
            &path,
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "plain-json"
out_dir = "a"

[[web.targets]]
name = "docs"
mode = "runtime"
out_dir = "b"
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        let err = config.validate_web_targets().expect_err("duplicate");
        assert_eq!(
            err,
            WebTargetConfigError::DuplicateTargetName("docs".to_string())
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn validates_docusaurus_required_paths() {
        let path = temp_path("web_docusaurus");
        fs::write(
            &path,
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "docusaurus"
out_dir = "src/generated/mf2"
docusaurus_i18n_dir = "i18n"
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        let err = config.validate_web_targets().expect_err("docs path");
        assert_eq!(
            err,
            WebTargetConfigError::MissingDocusaurusDocsSourceDir {
                target: "docs".to_string()
            }
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn accepts_hybrid_mode_for_web_frameworks() {
        let path = temp_path("web_hybrid");
        fs::write(
            &path,
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "app"
mode = "hybrid"
framework = "vite"
out_dir = "src/generated/mf2"
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        let target = config.web.target("app").expect("target");
        assert_eq!(target.mode, WebTargetMode::Hybrid);
        assert!(config.validate_web_targets().is_ok());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_hybrid_docusaurus_framework() {
        let path = temp_path("web_hybrid_docusaurus");
        fs::write(
            &path,
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "hybrid"
framework = "docusaurus"
out_dir = "src/generated/mf2"
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        let err = config
            .validate_web_targets()
            .expect_err("hybrid docusaurus");
        assert_eq!(
            err,
            WebTargetConfigError::HybridDocusaurusFramework {
                target: "docs".to_string()
            }
        );

        fs::remove_file(&path).ok();
    }
}
