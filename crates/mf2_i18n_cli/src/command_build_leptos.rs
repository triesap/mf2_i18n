use std::path::PathBuf;

use mf2_i18n_build::{LeptosTargetBuildError, LeptosTargetBuildOptions, build_leptos_target};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildLeptosCommandError {
    #[error(transparent)]
    Build(#[from] LeptosTargetBuildError),
}

#[derive(Debug, Clone)]
pub struct BuildLeptosOptions {
    pub config_path: PathBuf,
    pub target_name: String,
    pub out_dir: Option<PathBuf>,
}

pub fn run_build_leptos(options: &BuildLeptosOptions) -> Result<(), BuildLeptosCommandError> {
    let mut build_options =
        LeptosTargetBuildOptions::new(&options.config_path, &options.target_name);
    if let Some(out_dir) = &options.out_dir {
        build_options = build_options.with_out_dir(out_dir);
    }
    build_leptos_target(&build_options)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BuildLeptosOptions, run_build_leptos};
    use mf2_i18n_build::LeptosTargetBuildError;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        path.push(format!(
            "mf2_i18n_build_leptos_{}_{}",
            std::process::id(),
            id
        ));
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
    fn builds_leptos_target() {
        let dir = temp_dir();
        let locale_dir = dir.join("locales/en");
        fs::create_dir_all(&locale_dir).expect("locale");
        write_catalog(
            locale_dir.join("homepage.json"),
            &[("title", "Radroots"), ("welcome", "Welcome")],
        );
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[native.targets]]\nname = \"app-native\"\nout_dir = \"target/mf2/native\"\nmodule_name = \"native_i18n\"\n\n[[leptos.targets]]\nname = \"app\"\nnative_target = \"app-native\"\nout_dir = \"src/generated/i18n\"\nmodule_name = \"app_i18n\"\ndocument_title_key = \"homepage.title\"\nsync_html_lang = true\n",
        )
        .expect("config");

        let out_dir = dir.join("out");
        run_build_leptos(&BuildLeptosOptions {
            config_path,
            target_name: "app".to_string(),
            out_dir: Some(out_dir.clone()),
        })
        .expect("build");

        let generated_module = fs::read_to_string(out_dir.join("app_i18n.rs")).expect("module");
        assert!(generated_module.contains("include!(\"native_i18n/generated_module.rs\")"));
        assert!(generated_module.contains("LEPTOS_DOCUMENT_TITLE_KEY"));
        assert!(out_dir.join("native_i18n/generated_module.rs").exists());
        assert!(out_dir.join("native_i18n/packs/en.mf2pack").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_missing_leptos_target() {
        let dir = temp_dir();
        let locale_dir = dir.join("locales/en");
        fs::create_dir_all(&locale_dir).expect("locale");
        write_catalog(locale_dir.join("homepage.json"), &[("title", "Radroots")]);
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[native.targets]]\nname = \"app-native\"\nout_dir = \"target/mf2/native\"\nmodule_name = \"native_i18n\"\n",
        )
        .expect("config");

        let err = run_build_leptos(&BuildLeptosOptions {
            config_path,
            target_name: "app".to_string(),
            out_dir: Some(dir.join("out")),
        })
        .expect_err("target should fail");

        let super::BuildLeptosCommandError::Build(error) = err;
        assert!(matches!(
            error,
            LeptosTargetBuildError::MissingLeptosTarget(target) if target == "app"
        ));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_unknown_native_target_from_config() {
        let dir = temp_dir();
        let locale_dir = dir.join("locales/en");
        fs::create_dir_all(&locale_dir).expect("locale");
        write_catalog(locale_dir.join("homepage.json"), &[("title", "Radroots")]);
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[leptos.targets]]\nname = \"app\"\nnative_target = \"missing-native\"\nout_dir = \"src/generated/i18n\"\nmodule_name = \"app_i18n\"\n",
        )
        .expect("config");

        let err = run_build_leptos(&BuildLeptosOptions {
            config_path,
            target_name: "app".to_string(),
            out_dir: Some(dir.join("out")),
        })
        .expect_err("native target should fail");

        assert!(
            err.to_string()
                .contains("leptos target app references unknown native target missing-native")
        );

        fs::remove_dir_all(&dir).ok();
    }
}
