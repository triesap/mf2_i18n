use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::BuildIoError;

pub const PROJECT_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub version: u32,
    pub default_locale: String,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
    pub targets: Vec<ProjectTargetConfig>,
    #[serde(default)]
    pub source_dirs: Vec<String>,
    #[serde(default = "default_micro_locales_registry")]
    pub micro_locales_registry: Option<String>,
    #[serde(default = "default_project_salt_path")]
    pub project_salt_path: String,
    #[serde(default)]
    pub native: NativeConfig,
    #[serde(default)]
    pub leptos: LeptosConfig,
    #[serde(default)]
    pub web: WebConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SourceConfig {
    pub name: String,
    pub layout: SourceLayout,
    pub root: String,
    #[serde(default)]
    pub namespace_glob: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceLayout {
    LocaleNamespace,
    NamespaceLocale,
    SingleLocaleFile,
    DocusaurusDocs,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectTargetConfig {
    pub name: String,
    pub source: String,
    pub preset: WebPreset,
    pub out_dir: String,
    #[serde(default)]
    pub typescript: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NativeConfig {
    #[serde(default)]
    pub targets: Vec<NativeTargetConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeTargetConfig {
    pub name: String,
    pub out_dir: String,
    pub module_name: String,
    #[serde(default)]
    pub module_macro_path: Option<String>,
    #[serde(default)]
    pub release_id: Option<String>,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub namespaces: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LeptosConfig {
    #[serde(default)]
    pub targets: Vec<LeptosTargetConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeptosTargetConfig {
    pub name: String,
    pub native_target: String,
    pub out_dir: String,
    pub module_name: String,
    #[serde(default)]
    pub document_title_key: Option<String>,
    #[serde(default)]
    pub sync_html_lang: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebPreset {
    Vanilla,
    Vite,
    React,
    Svelte,
    Sveltekit,
    Next,
    Astro,
    Docusaurus,
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
    Sveltekit,
    Vite,
    Next,
    Astro,
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectConfigError {
    #[error("project config version is required")]
    MissingVersion,
    #[error("unsupported project config version {found}; expected {expected}")]
    UnsupportedVersion { found: u32, expected: u32 },
    #[error("default_locale must not be empty")]
    EmptyDefaultLocale,
    #[error("locale entries must not be empty")]
    EmptyLocale,
    #[error("duplicate locale {0}")]
    DuplicateLocale(String),
    #[error("default_locale {0} must be listed in locales")]
    DefaultLocaleNotListed(String),
    #[error("project config must declare at least one source")]
    NoSources,
    #[error("source name must not be empty")]
    EmptySourceName,
    #[error("duplicate source name {0}")]
    DuplicateSourceName(String),
    #[error("source {source_name} root must not be empty")]
    EmptySourceRoot { source_name: String },
    #[error("target name must not be empty")]
    EmptyTargetName,
    #[error("duplicate target name {0}")]
    DuplicateTargetName(String),
    #[error("target {target} source must not be empty")]
    EmptyTargetSource { target: String },
    #[error("target {target} references unknown source {source_name}")]
    UnknownTargetSource { target: String, source_name: String },
    #[error("target {target} out_dir must not be empty")]
    EmptyTargetOutDir { target: String },
    #[error("native target name must not be empty")]
    EmptyNativeTargetName,
    #[error("duplicate native target name {0}")]
    DuplicateNativeTargetName(String),
    #[error("native target {target} out_dir must not be empty")]
    EmptyNativeTargetOutDir { target: String },
    #[error("native target {target} module_name must not be empty")]
    EmptyNativeModuleName { target: String },
    #[error("native target {target} module_macro_path must not be empty when provided")]
    EmptyNativeModuleMacroPath { target: String },
    #[error("native target {target} release_id must not be empty when provided")]
    EmptyNativeReleaseId { target: String },
    #[error("native target {target} generated_at must not be empty when provided")]
    EmptyNativeGeneratedAt { target: String },
    #[error("native target {target} locale entries must not be empty")]
    EmptyNativeLocale { target: String },
    #[error("native target {target} namespace entries must not be empty")]
    EmptyNativeNamespace { target: String },
    #[error("leptos target name must not be empty")]
    EmptyLeptosTargetName,
    #[error("duplicate leptos target name {0}")]
    DuplicateLeptosTargetName(String),
    #[error("leptos target {target} references duplicate rust target name")]
    DuplicateRustTargetName { target: String },
    #[error("leptos target {target} native_target must not be empty")]
    EmptyLeptosNativeTarget { target: String },
    #[error("leptos target {target} references unknown native target {native_target}")]
    UnknownLeptosNativeTarget {
        target: String,
        native_target: String,
    },
    #[error("leptos target {target} out_dir must not be empty")]
    EmptyLeptosTargetOutDir { target: String },
    #[error("leptos target {target} module_name must not be empty")]
    EmptyLeptosModuleName { target: String },
    #[error("leptos target {target} document_title_key must not be empty when provided")]
    EmptyLeptosDocumentTitleKey { target: String },
    #[error(transparent)]
    Web(#[from] WebTargetConfigError),
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: PROJECT_CONFIG_VERSION,
            default_locale: "en".to_string(),
            locales: vec!["en".to_string()],
            sources: vec![SourceConfig {
                name: "default".to_string(),
                layout: SourceLayout::LocaleNamespace,
                root: "locales".to_string(),
                namespace_glob: Some("*.mf2".to_string()),
            }],
            targets: Vec::new(),
            source_dirs: vec!["locales".to_string()],
            micro_locales_registry: Some("micro-locales.toml".to_string()),
            project_salt_path: "id_salt.txt".to_string(),
            native: NativeConfig::default(),
            leptos: LeptosConfig::default(),
            web: WebConfig::default(),
        }
    }
}

impl ProjectConfig {
    pub fn validate(&self) -> Result<(), ProjectConfigError> {
        validate_version(self.version)?;
        validate_locales(&self.default_locale, &self.locales)?;
        validate_sources(&self.sources)?;
        validate_targets(&self.targets, &self.sources)?;
        self.native.validate()?;
        self.leptos.validate(&self.native)?;
        self.web.validate()?;
        Ok(())
    }

    pub fn validate_web_targets(&self) -> Result<(), WebTargetConfigError> {
        self.web.validate()
    }

    fn normalize(&mut self) -> Result<(), ProjectConfigError> {
        validate_version(self.version)?;
        if self.locales.is_empty() {
            self.locales.push(self.default_locale.clone());
        }
        if self.sources.is_empty() && !self.source_dirs.is_empty() {
            self.sources = self
                .source_dirs
                .iter()
                .enumerate()
                .map(|(index, root)| SourceConfig {
                    name: if index == 0 {
                        "default".to_string()
                    } else {
                        format!("source-{}", index + 1)
                    },
                    layout: SourceLayout::LocaleNamespace,
                    root: root.clone(),
                    namespace_glob: Some("*.mf2".to_string()),
                })
                .collect();
        }
        if self.source_dirs.is_empty() {
            self.source_dirs = self
                .sources
                .iter()
                .map(|source| source.root.clone())
                .collect();
        }
        if self.web.targets.is_empty() && !self.targets.is_empty() {
            self.web.targets = self
                .targets
                .iter()
                .map(WebTargetConfig::from_public_target)
                .collect();
        }
        self.validate()
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

impl NativeConfig {
    pub fn target(&self, name: &str) -> Option<&NativeTargetConfig> {
        self.targets.iter().find(|target| target.name == name)
    }

    pub fn validate(&self) -> Result<(), ProjectConfigError> {
        let mut names = std::collections::BTreeSet::new();
        for target in &self.targets {
            target.validate()?;
            if !names.insert(target.name.clone()) {
                return Err(ProjectConfigError::DuplicateNativeTargetName(
                    target.name.clone(),
                ));
            }
        }
        Ok(())
    }
}

impl NativeTargetConfig {
    pub fn resolve_out_dir(&self, config_path: &Path) -> PathBuf {
        resolve_config_relative_path(config_path, &self.out_dir)
    }

    pub fn module_macro_path(&self) -> Option<&str> {
        self.module_macro_path.as_deref()
    }

    fn validate(&self) -> Result<(), ProjectConfigError> {
        if self.name.trim().is_empty() {
            return Err(ProjectConfigError::EmptyNativeTargetName);
        }
        if self.out_dir.trim().is_empty() {
            return Err(ProjectConfigError::EmptyNativeTargetOutDir {
                target: self.name.clone(),
            });
        }
        if self.module_name.trim().is_empty() {
            return Err(ProjectConfigError::EmptyNativeModuleName {
                target: self.name.clone(),
            });
        }
        if self
            .module_macro_path
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ProjectConfigError::EmptyNativeModuleMacroPath {
                target: self.name.clone(),
            });
        }
        if self
            .release_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ProjectConfigError::EmptyNativeReleaseId {
                target: self.name.clone(),
            });
        }
        if self
            .generated_at
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ProjectConfigError::EmptyNativeGeneratedAt {
                target: self.name.clone(),
            });
        }
        if self.locales.iter().any(|locale| locale.trim().is_empty()) {
            return Err(ProjectConfigError::EmptyNativeLocale {
                target: self.name.clone(),
            });
        }
        if self
            .namespaces
            .iter()
            .any(|namespace| namespace.trim().is_empty())
        {
            return Err(ProjectConfigError::EmptyNativeNamespace {
                target: self.name.clone(),
            });
        }
        Ok(())
    }
}

impl LeptosConfig {
    pub fn target(&self, name: &str) -> Option<&LeptosTargetConfig> {
        self.targets.iter().find(|target| target.name == name)
    }

    pub fn validate(&self, native: &NativeConfig) -> Result<(), ProjectConfigError> {
        let native_names = native
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut names = std::collections::BTreeSet::new();
        for target in &self.targets {
            target.validate(&native_names)?;
            if !names.insert(target.name.clone()) {
                return Err(ProjectConfigError::DuplicateLeptosTargetName(
                    target.name.clone(),
                ));
            }
            if native_names.contains(target.name.as_str()) {
                return Err(ProjectConfigError::DuplicateRustTargetName {
                    target: target.name.clone(),
                });
            }
        }
        Ok(())
    }
}

impl LeptosTargetConfig {
    pub fn resolve_out_dir(&self, config_path: &Path) -> PathBuf {
        resolve_config_relative_path(config_path, &self.out_dir)
    }

    fn validate(
        &self,
        native_names: &std::collections::BTreeSet<&str>,
    ) -> Result<(), ProjectConfigError> {
        if self.name.trim().is_empty() {
            return Err(ProjectConfigError::EmptyLeptosTargetName);
        }
        if self.native_target.trim().is_empty() {
            return Err(ProjectConfigError::EmptyLeptosNativeTarget {
                target: self.name.clone(),
            });
        }
        if !native_names.contains(self.native_target.as_str()) {
            return Err(ProjectConfigError::UnknownLeptosNativeTarget {
                target: self.name.clone(),
                native_target: self.native_target.clone(),
            });
        }
        if self.out_dir.trim().is_empty() {
            return Err(ProjectConfigError::EmptyLeptosTargetOutDir {
                target: self.name.clone(),
            });
        }
        if self.module_name.trim().is_empty() {
            return Err(ProjectConfigError::EmptyLeptosModuleName {
                target: self.name.clone(),
            });
        }
        if self
            .document_title_key
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ProjectConfigError::EmptyLeptosDocumentTitleKey {
                target: self.name.clone(),
            });
        }
        Ok(())
    }
}

impl WebTargetConfig {
    fn from_public_target(target: &ProjectTargetConfig) -> Self {
        let (mode, framework) = target.preset.web_target_shape();
        Self {
            name: target.name.clone(),
            mode,
            framework,
            out_dir: target.out_dir.clone(),
            asset_dir: None,
            locales: Vec::new(),
            namespaces: Vec::new(),
            docs_source_dir: target.preset.is_docusaurus().then(|| "docs".to_string()),
            docusaurus_i18n_dir: target.preset.is_docusaurus().then(|| "i18n".to_string()),
            docusaurus_sidebar_id: None,
            generated_at: None,
            release_id: None,
            stale_policy: None,
        }
    }

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

impl WebPreset {
    fn web_target_shape(self) -> (WebTargetMode, Option<WebFramework>) {
        match self {
            Self::Vanilla => (WebTargetMode::PlainJson, Some(WebFramework::Vanilla)),
            Self::Vite => (WebTargetMode::Hybrid, Some(WebFramework::Vite)),
            Self::React => (WebTargetMode::Hybrid, Some(WebFramework::React)),
            Self::Svelte => (WebTargetMode::Hybrid, Some(WebFramework::Svelte)),
            Self::Sveltekit => (WebTargetMode::Hybrid, Some(WebFramework::Sveltekit)),
            Self::Next => (WebTargetMode::Hybrid, Some(WebFramework::Next)),
            Self::Astro => (WebTargetMode::Hybrid, Some(WebFramework::Astro)),
            Self::Docusaurus => (WebTargetMode::Docusaurus, Some(WebFramework::Docusaurus)),
        }
    }

    fn is_docusaurus(self) -> bool {
        matches!(self, Self::Docusaurus)
    }
}

fn default_micro_locales_registry() -> Option<String> {
    Some("micro-locales.toml".to_string())
}

fn default_project_salt_path() -> String {
    "id_salt.txt".to_string()
}

fn resolve_config_relative_path(config_path: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

fn validate_version(version: u32) -> Result<(), ProjectConfigError> {
    if version == 0 {
        return Err(ProjectConfigError::MissingVersion);
    }
    if version != PROJECT_CONFIG_VERSION {
        return Err(ProjectConfigError::UnsupportedVersion {
            found: version,
            expected: PROJECT_CONFIG_VERSION,
        });
    }
    Ok(())
}

fn validate_locales(default_locale: &str, locales: &[String]) -> Result<(), ProjectConfigError> {
    if default_locale.trim().is_empty() {
        return Err(ProjectConfigError::EmptyDefaultLocale);
    }
    if locales.iter().any(|locale| locale.trim().is_empty()) {
        return Err(ProjectConfigError::EmptyLocale);
    }
    let mut seen = std::collections::BTreeSet::new();
    for locale in locales {
        if !seen.insert(locale.clone()) {
            return Err(ProjectConfigError::DuplicateLocale(locale.clone()));
        }
    }
    if !locales.iter().any(|locale| locale == default_locale) {
        return Err(ProjectConfigError::DefaultLocaleNotListed(
            default_locale.to_string(),
        ));
    }
    Ok(())
}

fn validate_sources(sources: &[SourceConfig]) -> Result<(), ProjectConfigError> {
    if sources.is_empty() {
        return Err(ProjectConfigError::NoSources);
    }
    let mut names = std::collections::BTreeSet::new();
    for source in sources {
        if source.name.trim().is_empty() {
            return Err(ProjectConfigError::EmptySourceName);
        }
        if source.root.trim().is_empty() {
            return Err(ProjectConfigError::EmptySourceRoot {
                source_name: source.name.clone(),
            });
        }
        if !names.insert(source.name.clone()) {
            return Err(ProjectConfigError::DuplicateSourceName(source.name.clone()));
        }
    }
    Ok(())
}

fn validate_targets(
    targets: &[ProjectTargetConfig],
    sources: &[SourceConfig],
) -> Result<(), ProjectConfigError> {
    let source_names = sources
        .iter()
        .map(|source| source.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut target_names = std::collections::BTreeSet::new();
    for target in targets {
        if target.name.trim().is_empty() {
            return Err(ProjectConfigError::EmptyTargetName);
        }
        if !target_names.insert(target.name.clone()) {
            return Err(ProjectConfigError::DuplicateTargetName(target.name.clone()));
        }
        if target.source.trim().is_empty() {
            return Err(ProjectConfigError::EmptyTargetSource {
                target: target.name.clone(),
            });
        }
        if !source_names.contains(target.source.as_str()) {
            return Err(ProjectConfigError::UnknownTargetSource {
                target: target.name.clone(),
                source_name: target.source.clone(),
            });
        }
        if target.out_dir.trim().is_empty() {
            return Err(ProjectConfigError::EmptyTargetOutDir {
                target: target.name.clone(),
            });
        }
    }
    Ok(())
}

pub fn load_project_config(path: &Path) -> Result<ProjectConfig, BuildIoError> {
    let contents = fs::read_to_string(path)?;
    let mut config: ProjectConfig = toml::from_str(&contents)?;
    config
        .normalize()
        .map_err(|err| BuildIoError::Config(err.to_string()))?;
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
        PROJECT_CONFIG_VERSION, ProjectConfig, WebFramework, WebStalePolicy, WebTargetMode,
        load_project_config_or_default,
    };
    use crate::error::BuildIoError;
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
version = 1
default_locale = "fr"
source_dirs = ["locales"]
micro_locales_registry = "micro-locales.toml"
project_salt_path = "id_salt.txt"
"#;
        fs::write(&path, contents).expect("write");
        let config = load_project_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "fr");
        assert_eq!(config.version, PROJECT_CONFIG_VERSION);
        assert_eq!(config.locales, ["fr".to_string()]);
        assert_eq!(config.sources[0].root, "locales");
        assert!(config.web.targets.is_empty());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn default_values_are_stable() {
        let config = ProjectConfig::default();
        assert_eq!(config.version, PROJECT_CONFIG_VERSION);
        assert_eq!(config.project_salt_path, "id_salt.txt");
        assert_eq!(config.sources[0].root, "locales");
        assert!(config.native.targets.is_empty());
        assert!(config.leptos.targets.is_empty());
        assert!(config.web.targets.is_empty());
        assert!(config.validate().is_ok());
        assert!(config.validate_web_targets().is_ok());
    }

    #[test]
    fn loads_public_v1_config_contract() {
        let path = temp_path("public_v1");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
locales = ["en", "es"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
namespace_glob = "*.mf2"

[[targets]]
name = "web"
source = "site"
preset = "react"
out_dir = "src/generated/mf2"
typescript = true
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        assert_eq!(config.project_salt_path, "id_salt.txt");
        assert_eq!(config.source_dirs, ["locales".to_string()]);
        let target = config.web.target("web").expect("target");
        assert_eq!(target.mode, WebTargetMode::Hybrid);
        assert_eq!(target.framework, Some(WebFramework::React));
        assert!(config.validate().is_ok());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn loads_native_and_leptos_targets_with_config_relative_paths() {
        let path = temp_path("rust_targets");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[native.targets]]
name = "app-native"
out_dir = "target/mf2/native"
module_name = "app_i18n"
module_macro_path = "mf2_i18n::define_i18n_module!"
release_id = "app-local"
generated_at = "2026-06-28T00:00:00Z"
locales = ["en"]
namespaces = ["common"]

[[leptos.targets]]
name = "app-leptos"
native_target = "app-native"
out_dir = "src/generated/i18n"
module_name = "app_i18n"
document_title_key = "common.home.title"
sync_html_lang = true
"#,
        )
        .expect("write");

        let config = load_project_config_or_default(&path).expect("config");
        let native = config.native.target("app-native").expect("native");
        assert_eq!(native.module_name, "app_i18n");
        assert_eq!(
            native.resolve_out_dir(&path),
            path.parent().expect("parent").join("target/mf2/native")
        );
        let leptos = config.leptos.target("app-leptos").expect("leptos");
        assert_eq!(leptos.native_target, "app-native");
        assert!(leptos.sync_html_lang);
        assert_eq!(
            leptos.resolve_out_dir(&path),
            path.parent().expect("parent").join("src/generated/i18n")
        );
        assert!(config.validate().is_ok());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_leptos_targets_with_unknown_native_target() {
        let path = temp_path("unknown_leptos_native");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[leptos.targets]]
name = "app"
native_target = "missing"
out_dir = "src/generated/i18n"
module_name = "app_i18n"
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("native target");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "leptos target app references unknown native target missing"
        ));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_duplicate_native_and_leptos_rust_target_names() {
        let path = temp_path("duplicate_rust_target");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[native.targets]]
name = "app"
out_dir = "target/mf2/native"
module_name = "app_i18n"

[[leptos.targets]]
name = "app"
native_target = "app"
out_dir = "src/generated/i18n"
module_name = "app_i18n"
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("duplicate rust target");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "leptos target app references duplicate rust target name"
        ));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_empty_native_and_leptos_target_fields() {
        let path = temp_path("empty_native_module");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[native.targets]]
name = "app"
out_dir = "target/mf2/native"
module_name = ""
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("empty native module");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "native target app module_name must not be empty"
        ));
        fs::remove_file(&path).ok();

        let path = temp_path("empty_leptos_title");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[native.targets]]
name = "app-native"
out_dir = "target/mf2/native"
module_name = "app_i18n"

[[leptos.targets]]
name = "app"
native_target = "app-native"
out_dir = "src/generated/i18n"
module_name = "app_i18n"
document_title_key = ""
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("empty title key");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "leptos target app document_title_key must not be empty when provided"
        ));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_missing_config_version() {
        let path = temp_path("missing_version");
        fs::write(
            &path,
            r#"
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("version error");
        assert!(
            matches!(err, BuildIoError::Config(message) if message == "project config version is required")
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_unknown_config_version() {
        let path = temp_path("unknown_version");
        fs::write(
            &path,
            r#"
version = 2
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("version error");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "unsupported project config version 2; expected 1"
        ));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_unknown_target_source() {
        let path = temp_path("unknown_source");
        fs::write(
            &path,
            r#"
version = 1
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"

[[targets]]
name = "web"
source = "missing"
preset = "vite"
out_dir = "src/generated/mf2"
"#,
        )
        .expect("write");

        let err = load_project_config_or_default(&path).expect_err("source error");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "target web references unknown source missing"
        ));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn loads_web_targets() {
        let path = temp_path("web");
        fs::write(
            &path,
            r#"
version = 1
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
version = 1
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

        let err = load_project_config_or_default(&path).expect_err("duplicate");
        assert!(
            matches!(err, BuildIoError::Config(message) if message == "duplicate web target name docs")
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn validates_docusaurus_required_paths() {
        let path = temp_path("web_docusaurus");
        fs::write(
            &path,
            r#"
version = 1
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

        let err = load_project_config_or_default(&path).expect_err("docs path");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "web target docs docusaurus mode requires docs_source_dir"
        ));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn accepts_hybrid_mode_for_web_frameworks() {
        let path = temp_path("web_hybrid");
        fs::write(
            &path,
            r#"
version = 1
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
version = 1
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

        let err = load_project_config_or_default(&path).expect_err("hybrid docusaurus");
        assert!(matches!(
            err,
            BuildIoError::Config(message)
                if message == "web target docs hybrid mode does not support docusaurus framework"
        ));

        fs::remove_file(&path).ok();
    }
}
