use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::artifacts::{write_id_map, write_id_map_hash};
use crate::compiler::{CompileError, compile_message};
use crate::id_map::{IdMap, IdMapError, build_id_map};
use crate::pack_encode::{PackBuildInput, encode_pack};
use crate::parser::parse_message;
use crate::project::{ProjectError, ProjectLayout};

const DEFAULT_MODULE_MACRO_PATH: &str = "mf2_i18n::define_i18n_module!";
const GENERATED_MODULE_FILE: &str = "generated_module.rs";
const GENERATED_CATALOG_FILE: &str = "generated_catalog.rs";
const ID_MAP_JSON_FILE: &str = "id-map.json";
const ID_MAP_HASH_FILE: &str = "id-map.sha256";

type Catalog = BTreeMap<String, String>;

#[derive(Debug, Clone)]
pub struct NativeModuleBuildOptions {
    config_path: PathBuf,
    out_dir: PathBuf,
    artifact_dir_name: String,
    module_macro_path: String,
}

impl NativeModuleBuildOptions {
    pub fn new(
        config_path: impl Into<PathBuf>,
        out_dir: impl Into<PathBuf>,
        artifact_dir_name: impl Into<String>,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            out_dir: out_dir.into(),
            artifact_dir_name: artifact_dir_name.into(),
            module_macro_path: DEFAULT_MODULE_MACRO_PATH.to_owned(),
        }
    }

    pub fn with_module_macro_path(mut self, module_macro_path: impl Into<String>) -> Self {
        self.module_macro_path = module_macro_path.into();
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn artifact_dir_name(&self) -> &str {
        &self.artifact_dir_name
    }

    pub fn module_macro_path(&self) -> &str {
        &self.module_macro_path
    }
}

#[derive(Debug, Clone)]
pub struct NativeModuleBuildOutput {
    artifact_dir: PathBuf,
    generated_module_path: PathBuf,
    generated_catalog_path: PathBuf,
    rerun_if_changed_paths: Vec<PathBuf>,
    default_locale: String,
    supported_locales: Vec<String>,
    default_catalog_keys: Vec<String>,
}

impl NativeModuleBuildOutput {
    pub fn artifact_dir(&self) -> &Path {
        &self.artifact_dir
    }

    pub fn generated_module_path(&self) -> &Path {
        &self.generated_module_path
    }

    pub fn generated_catalog_path(&self) -> &Path {
        &self.generated_catalog_path
    }

    pub fn rerun_if_changed_paths(&self) -> &[PathBuf] {
        &self.rerun_if_changed_paths
    }

    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    pub fn supported_locales(&self) -> &[String] {
        &self.supported_locales
    }

    pub fn default_catalog_keys(&self) -> &[String] {
        &self.default_catalog_keys
    }
}

#[derive(Debug, Error)]
pub enum NativeModuleBuildError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    IdMap(#[from] IdMapError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("artifact dir name must not be empty")]
    EmptyArtifactDirName,
    #[error("artifact dir name must be a single relative path segment: {0}")]
    InvalidArtifactDirName(String),
    #[error("module macro path must not be empty")]
    EmptyModuleMacroPath,
    #[error("project config must declare at least one source dir")]
    NoSourceDirs,
    #[error("at least one locale catalog is required")]
    NoLocaleCatalogs,
    #[error("default locale {0} catalog should exist")]
    MissingDefaultLocale(String),
    #[error("duplicate i18n message key {key} in locale {locale} from {path}")]
    DuplicateMessageKey {
        key: String,
        locale: String,
        path: PathBuf,
    },
    #[error(
        "i18n catalog keys for locale {locale} do not match reference locale {reference_locale}"
    )]
    CatalogKeyMismatch {
        locale: String,
        reference_locale: String,
    },
    #[error(
        "failed to parse i18n message for locale {locale} key {key}: {message} at {line}:{column}"
    )]
    Parse {
        locale: String,
        key: String,
        message: String,
        line: u32,
        column: u32,
    },
    #[error("failed to compile i18n message for locale {locale} key {key}: {source}")]
    Compile {
        locale: String,
        key: String,
        #[source]
        source: CompileError,
    },
    #[error("missing message id for locale {locale} key {key}")]
    MissingMessageId { locale: String, key: String },
}

pub fn build_native_module(
    options: &NativeModuleBuildOptions,
) -> Result<NativeModuleBuildOutput, NativeModuleBuildError> {
    validate_options(options)?;

    let project = ProjectLayout::load_or_default(options.config_path())?;
    let mut rerun_paths = BTreeSet::from([options.config_path().to_path_buf()]);
    let salt_path = project.project_salt_path();
    rerun_paths.insert(salt_path);
    let id_salt = project.load_project_salt()?;

    let catalogs = load_catalogs(&project, &mut rerun_paths)?;
    ensure_catalog_keys_match(&catalogs)?;

    let default_locale = project.config().default_locale.clone();
    let default_catalog = catalogs
        .get(&default_locale)
        .ok_or_else(|| NativeModuleBuildError::MissingDefaultLocale(default_locale.clone()))?;
    let default_catalog_keys = default_catalog.keys().cloned().collect::<Vec<_>>();

    let id_map = build_id_map(default_catalog_keys.iter().cloned(), &id_salt)?;
    let id_map_hash = id_map.hash()?;

    let artifact_dir = options.out_dir().join(options.artifact_dir_name());
    fs::create_dir_all(&artifact_dir)?;
    write_id_map(&artifact_dir.join(ID_MAP_JSON_FILE), &id_map)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    write_id_map_hash(&artifact_dir.join(ID_MAP_HASH_FILE), id_map_hash)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let supported_locales = catalogs.keys().cloned().collect::<Vec<_>>();
    for (locale, catalog) in &catalogs {
        let pack_bytes = compile_catalog_pack(locale, catalog, &id_map, id_map_hash)?;
        fs::write(artifact_dir.join(format!("{locale}.mf2pack")), pack_bytes)?;
    }

    let generated_module_path = artifact_dir.join(GENERATED_MODULE_FILE);
    fs::write(
        &generated_module_path,
        render_generated_module(
            &default_locale,
            &supported_locales,
            options.artifact_dir_name(),
            options.module_macro_path(),
        ),
    )?;

    let generated_catalog_path = artifact_dir.join(GENERATED_CATALOG_FILE);
    fs::write(
        &generated_catalog_path,
        render_generated_catalog(&default_locale, &supported_locales, &default_catalog_keys),
    )?;

    Ok(NativeModuleBuildOutput {
        artifact_dir,
        generated_module_path,
        generated_catalog_path,
        rerun_if_changed_paths: rerun_paths.into_iter().collect(),
        default_locale,
        supported_locales,
        default_catalog_keys,
    })
}

fn validate_options(options: &NativeModuleBuildOptions) -> Result<(), NativeModuleBuildError> {
    if options.artifact_dir_name().trim().is_empty() {
        return Err(NativeModuleBuildError::EmptyArtifactDirName);
    }

    let components = Path::new(options.artifact_dir_name())
        .components()
        .collect::<Vec<_>>();
    if components.len() != 1 || !matches!(components.first(), Some(Component::Normal(_))) {
        return Err(NativeModuleBuildError::InvalidArtifactDirName(
            options.artifact_dir_name().to_owned(),
        ));
    }

    if options.module_macro_path().trim().is_empty() {
        return Err(NativeModuleBuildError::EmptyModuleMacroPath);
    }

    Ok(())
}

fn load_catalogs(
    project: &ProjectLayout,
    rerun_paths: &mut BTreeSet<PathBuf>,
) -> Result<BTreeMap<String, Catalog>, NativeModuleBuildError> {
    let source_roots = project.source_roots();
    if source_roots.is_empty() {
        return Err(NativeModuleBuildError::NoSourceDirs);
    }

    let mut catalogs = BTreeMap::<String, Catalog>::new();
    for source_root in source_roots {
        rerun_paths.insert(source_root.clone());
        let entries = fs::read_dir(&source_root)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let locale = entry.file_name().to_string_lossy().into_owned();
            let messages_path = path.join("messages.json");
            if !messages_path.is_file() {
                continue;
            }
            rerun_paths.insert(messages_path.clone());

            let catalog = load_catalog(&messages_path)?;
            let merged = catalogs.entry(locale.clone()).or_default();
            for (key, value) in catalog {
                if merged.insert(key.clone(), value).is_some() {
                    return Err(NativeModuleBuildError::DuplicateMessageKey {
                        key,
                        locale: locale.clone(),
                        path: messages_path.clone(),
                    });
                }
            }
        }
    }

    if catalogs.is_empty() {
        return Err(NativeModuleBuildError::NoLocaleCatalogs);
    }

    Ok(catalogs)
}

fn load_catalog(path: &Path) -> Result<Catalog, NativeModuleBuildError> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn ensure_catalog_keys_match(
    catalogs: &BTreeMap<String, Catalog>,
) -> Result<(), NativeModuleBuildError> {
    let Some((reference_locale, reference_catalog)) = catalogs.iter().next() else {
        return Err(NativeModuleBuildError::NoLocaleCatalogs);
    };

    let reference_keys = reference_catalog.keys().cloned().collect::<Vec<_>>();
    for (locale, catalog) in catalogs.iter().skip(1) {
        let keys = catalog.keys().cloned().collect::<Vec<_>>();
        if keys != reference_keys {
            return Err(NativeModuleBuildError::CatalogKeyMismatch {
                locale: locale.clone(),
                reference_locale: reference_locale.clone(),
            });
        }
    }

    Ok(())
}

fn compile_catalog_pack(
    locale: &str,
    catalog: &Catalog,
    id_map: &IdMap,
    id_map_hash: [u8; 32],
) -> Result<Vec<u8>, NativeModuleBuildError> {
    let mut messages = BTreeMap::new();

    for (key, source) in catalog {
        let parsed = parse_message(source).map_err(|error| NativeModuleBuildError::Parse {
            locale: locale.to_owned(),
            key: key.clone(),
            message: error.message,
            line: error.span.line,
            column: error.span.column,
        })?;
        let compiled =
            compile_message(&parsed).map_err(|source| NativeModuleBuildError::Compile {
                locale: locale.to_owned(),
                key: key.clone(),
                source,
            })?;
        let message_id =
            id_map
                .get(key)
                .ok_or_else(|| NativeModuleBuildError::MissingMessageId {
                    locale: locale.to_owned(),
                    key: key.clone(),
                })?;
        messages.insert(message_id, compiled.program);
    }

    Ok(encode_pack(&PackBuildInput {
        pack_kind: mf2_i18n_core::PackKind::Base,
        id_map_hash,
        locale_tag: locale.to_owned(),
        parent_tag: None,
        build_epoch_ms: 0,
        messages,
    }))
}

fn render_generated_module(
    default_locale: &str,
    supported_locales: &[String],
    artifact_dir_name: &str,
    module_macro_path: &str,
) -> String {
    let packs_source = supported_locales
        .iter()
        .map(|locale| {
            format!(
                "            ({locale:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{artifact_dir_name}/{locale}.mf2pack\"))),"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "mod generated {{\n    {module_macro_path} {{\n        init_policy: strict,\n        default_locale: {default_locale:?},\n        id_map_json: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{artifact_dir_name}/{ID_MAP_JSON_FILE}\")),\n        id_map_hash: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{artifact_dir_name}/{ID_MAP_HASH_FILE}\")),\n        packs: [\n{packs_source}\n        ],\n    }}\n}}\n"
    )
}

fn render_generated_catalog(
    default_locale: &str,
    supported_locales: &[String],
    default_catalog_keys: &[String],
) -> String {
    let supported_locale_values = supported_locales
        .iter()
        .map(|locale| format!("{locale:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let key_values = default_catalog_keys
        .iter()
        .map(|key| format!("{key:?}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "const DEFAULT_LOCALE_ID: &str = {default_locale:?};\nconst SUPPORTED_LOCALE_IDS: &[&str] = &[{supported_locale_values}];\n#[cfg(test)] const DEFAULT_CATALOG_KEY_IDS: &[&str] = &[{key_values}];\n"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ID_MAP_HASH_FILE, ID_MAP_JSON_FILE, NativeModuleBuildError, NativeModuleBuildOptions,
        build_native_module,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_native_module_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_catalog(path: &Path, messages: &[(&str, &str)]) {
        let contents = messages
            .iter()
            .map(|(key, value)| format!("  {key:?}: {value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        fs::write(path, format!("{{\n{contents}\n}}\n")).expect("write");
    }

    fn write_project_fixture(root: &Path) -> PathBuf {
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let french_dir = locales_root.join("fr");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        fs::create_dir_all(&french_dir).expect("french locale dir");

        write_catalog(
            &english_dir.join("messages.json"),
            &[
                ("home.title", "Hi"),
                ("home.subtitle", "Grow from the root"),
            ],
        );
        write_catalog(
            &french_dir.join("messages.json"),
            &[
                ("home.title", "Salut"),
                ("home.subtitle", "Cultiver depuis la racine"),
            ],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");
        config_path
    }

    #[test]
    fn builds_native_module_artifacts() {
        let root = temp_dir("success");
        let config_path = write_project_fixture(&root);
        let out_dir = root.join("out");

        let output = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            &out_dir,
            "app_i18n",
        ))
        .expect("build");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "fr".to_string()]
        );
        assert_eq!(
            output.default_catalog_keys(),
            &["home.subtitle".to_string(), "home.title".to_string()]
        );
        assert!(output.artifact_dir().join("en.mf2pack").exists());
        assert!(output.artifact_dir().join("fr.mf2pack").exists());
        assert!(output.artifact_dir().join(ID_MAP_JSON_FILE).exists());
        assert!(output.artifact_dir().join(ID_MAP_HASH_FILE).exists());
        assert!(output.generated_module_path().exists());
        assert!(output.generated_catalog_path().exists());
        assert!(output.rerun_if_changed_paths().contains(&config_path));
        assert!(
            output
                .rerun_if_changed_paths()
                .contains(&root.join("id_salt.txt"))
        );
        assert!(
            output
                .rerun_if_changed_paths()
                .contains(&root.join("locales").join("en").join("messages.json"))
        );
        assert!(
            output
                .rerun_if_changed_paths()
                .contains(&root.join("locales").join("fr").join("messages.json"))
        );

        let generated_module =
            fs::read_to_string(output.generated_module_path()).expect("generated module");
        assert!(generated_module.contains("mf2_i18n::define_i18n_module!"));
        assert!(generated_module.contains("/app_i18n/en.mf2pack"));

        let generated_catalog =
            fs::read_to_string(output.generated_catalog_path()).expect("generated catalog");
        assert!(generated_catalog.contains("DEFAULT_LOCALE_ID"));
        assert!(generated_catalog.contains("\"fr\""));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn supports_custom_module_macro_path() {
        let root = temp_dir("macro");
        let config_path = write_project_fixture(&root);
        let out_dir = root.join("out");

        let output = build_native_module(
            &NativeModuleBuildOptions::new(&config_path, &out_dir, "app_i18n")
                .with_module_macro_path("mf2_i18n_native::define_i18n_module!"),
        )
        .expect("build");

        let generated_module =
            fs::read_to_string(output.generated_module_path()).expect("generated module");
        assert!(generated_module.contains("mf2_i18n_native::define_i18n_module!"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_mismatched_catalog_keys() {
        let root = temp_dir("mismatch");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let french_dir = locales_root.join("fr");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        fs::create_dir_all(&french_dir).expect("french locale dir");

        write_catalog(&english_dir.join("messages.json"), &[("home.title", "Hi")]);
        write_catalog(
            &french_dir.join("messages.json"),
            &[("home.subtitle", "Salut")],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let err = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect_err("mismatch should fail");
        assert!(matches!(
            err,
            NativeModuleBuildError::CatalogKeyMismatch { locale, reference_locale }
                if locale == "fr" && reference_locale == "en"
        ));

        fs::remove_dir_all(&root).ok();
    }
}
