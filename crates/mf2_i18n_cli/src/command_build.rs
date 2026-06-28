use std::path::PathBuf;

use mf2_i18n_build::{
    NativeModuleBuildError, NativeModuleBuildOptions, ProjectError, ProjectLayout,
    build_native_module,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildCommandError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("missing native target {0}")]
    MissingTarget(String),
    #[error(transparent)]
    Build(#[from] NativeModuleBuildError),
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub config_path: PathBuf,
    pub target_name: String,
    pub out_dir: Option<PathBuf>,
}

pub fn run_build(options: &BuildOptions) -> Result<(), BuildCommandError> {
    let project = ProjectLayout::load_or_default(&options.config_path)?;
    let target = project
        .config()
        .native
        .target(&options.target_name)
        .ok_or_else(|| BuildCommandError::MissingTarget(options.target_name.clone()))?;
    let out_dir = options
        .out_dir
        .clone()
        .unwrap_or_else(|| target.resolve_out_dir(project.config_path()));
    let mut build_options =
        NativeModuleBuildOptions::new(&options.config_path, out_dir, &target.module_name);

    if !target.locales.is_empty() {
        build_options = build_options.with_locales(target.locales.clone());
    }

    if !target.namespaces.is_empty() {
        build_options = build_options.with_namespaces(target.namespaces.clone());
    }

    if let Some(module_macro_path) = target.module_macro_path() {
        build_options = build_options.with_module_macro_path(module_macro_path);
    }

    if target.release_id.is_some() || target.generated_at.is_some() {
        let release_id = target
            .release_id
            .clone()
            .unwrap_or_else(|| build_options.release_id().to_string());
        let generated_at = target
            .generated_at
            .clone()
            .unwrap_or_else(|| build_options.generated_at().to_string());
        build_options = build_options.with_release_metadata(release_id, generated_at);
    }

    build_native_module(&build_options)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BuildCommandError, BuildOptions, run_build};
    use mf2_i18n_build::NativeModuleBuildError;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        path.push(format!("mf2_i18n_build_{}_{}", std::process::id(), id));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_catalog(path: PathBuf, messages: &[(&str, &str)]) {
        let contents = messages
            .iter()
            .map(|(key, value)| format!("  {key:?}: {value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        fs::write(path, format!("{{\n{contents}\n}}\n")).expect("write");
    }

    #[test]
    fn builds_generated_native_module() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[native.targets]]\nname = \"app\"\nout_dir = \"generated/native\"\nmodule_name = \"app_i18n\"\nrelease_id = \"r1\"\ngenerated_at = \"2026-02-01T00:00:00Z\"\n",
        )
        .expect("config");

        let out_dir = dir.join("out");
        run_build(&BuildOptions {
            config_path,
            target_name: "app".to_string(),
            out_dir: Some(out_dir.clone()),
        })
        .expect("build");

        let artifact_dir = out_dir.join("app_i18n");
        assert!(artifact_dir.join("generated_module.rs").exists());
        assert!(artifact_dir.join("manifest.json").exists());
        assert!(artifact_dir.join("id-map.json").exists());
        assert!(artifact_dir.join("packs/en.mf2pack").exists());
        assert!(artifact_dir.join("platform-bundle.json").exists());
        assert!(
            fs::read_to_string(artifact_dir.join("id-map.json"))
                .expect("id map")
                .contains("\"common.home.title\"")
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_honors_native_target_filters() {
        let dir = temp_dir();
        for locale in ["en", "es", "fr"] {
            fs::create_dir_all(dir.join("locales").join(locale)).expect("locale");
        }
        write_catalog(
            dir.join("locales/en/common.json"),
            &[("home.title", "Hi"), ("cart.count", "{ $count :number }")],
        );
        write_catalog(
            dir.join("locales/en/admin.json"),
            &[("panel.title", "Admin")],
        );
        write_catalog(
            dir.join("locales/es/common.json"),
            &[("home.title", "Hola"), ("cart.count", "{ $count :number }")],
        );
        write_catalog(
            dir.join("locales/es/admin.json"),
            &[("panel.title", "Administracion")],
        );
        write_catalog(
            dir.join("locales/fr/common.json"),
            &[
                ("home.title", "Salut"),
                ("cart.count", "{ $count :number }"),
            ],
        );
        write_catalog(
            dir.join("locales/fr/admin.json"),
            &[("panel.title", "Admin")],
        );
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[native.targets]]\nname = \"app\"\nout_dir = \"generated/native\"\nmodule_name = \"app_i18n\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"common\"]\n",
        )
        .expect("config");

        let out_dir = dir.join("out");
        run_build(&BuildOptions {
            config_path,
            target_name: "app".to_string(),
            out_dir: Some(out_dir.clone()),
        })
        .expect("build");

        let artifact_dir = out_dir.join("app_i18n");
        assert!(artifact_dir.join("packs/en.mf2pack").exists());
        assert!(artifact_dir.join("packs/es.mf2pack").exists());
        assert!(!artifact_dir.join("packs/fr.mf2pack").exists());

        let id_map_json = fs::read_to_string(artifact_dir.join("id-map.json")).expect("id map");
        assert!(id_map_json.contains("\"common.home.title\""));
        assert!(!id_map_json.contains("\"admin.panel.title\""));

        let generated_module =
            fs::read_to_string(artifact_dir.join("generated_module.rs")).expect("module");
        assert!(generated_module.contains("CommonHomeTitle"));
        assert!(generated_module.contains("CommonCartCount"));
        assert!(!generated_module.contains("AdminPanelTitle"));
        assert!(
            generated_module.contains("pub const SUPPORTED_LOCALES: &[&str] = &[\"en\", \"es\"]")
        );
        assert!(generated_module.contains("/app_i18n/packs/en.mf2pack"));
        assert!(generated_module.contains("/app_i18n/packs/es.mf2pack"));
        assert!(!generated_module.contains("/app_i18n/packs/fr.mf2pack"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_rejects_unknown_formatter() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.mf2"),
            "home.title = { $value :weird }",
        )
        .expect("write");
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[native.targets]]\nname = \"app\"\nout_dir = \"generated/native\"\nmodule_name = \"app_i18n\"\n",
        )
        .expect("config");

        let err = run_build(&BuildOptions {
            config_path,
            target_name: "app".to_string(),
            out_dir: Some(dir.join("out")),
        })
        .expect_err("build should fail");

        assert!(matches!(
            err,
            BuildCommandError::Build(NativeModuleBuildError::Compile { key, .. })
                if key == "common.home.title"
        ));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_rejects_missing_native_target() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"",
        )
        .expect("config");

        let err = run_build(&BuildOptions {
            config_path,
            target_name: "missing".to_string(),
            out_dir: None,
        })
        .expect_err("target should fail");

        assert!(matches!(err, BuildCommandError::MissingTarget(target) if target == "missing"));

        fs::remove_dir_all(&dir).ok();
    }
}
