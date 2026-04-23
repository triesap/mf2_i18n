#[cfg(feature = "build")]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use mf2_i18n::build::{
        NativeModuleBuildOptions, ProjectRuntimeBuildOptions, WebJsonExportOptions,
        build_native_module, export_web_json,
    };
    use mf2_i18n::{
        BuildIoError, CompileError, PlatformBundleManifest, ProjectConfig, ProjectLayout,
        WebJsonMode, load_project_config_or_default, resolve_config_relative_path,
    };

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_facade_{name}_{nanos}.toml"));
        path
    }

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_facade_build_{name}_{nanos}"));
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

    #[test]
    fn root_exports_build_surface() {
        let path = temp_path("missing");
        let config = load_project_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "en");

        let _compile_error: Option<CompileError> = None;
        let _io_error: Option<BuildIoError> = None;
        let _layout: Option<ProjectLayout> = None;
        let _manifest: Option<PlatformBundleManifest> = None;
        let _runtime_options = ProjectRuntimeBuildOptions::new(
            &path,
            std::env::temp_dir(),
            "r1",
            "2026-02-01T00:00:00Z",
        );
        let _web_json_options =
            WebJsonExportOptions::new(&path, std::env::temp_dir()).with_mode(WebJsonMode::Plain);
        let project_root = resolve_config_relative_path(&path, "locales");
        assert!(project_root.ends_with("locales"));

        let default_config = ProjectConfig::default();
        assert_eq!(default_config.project_salt_path, "id_salt.txt");
    }

    #[test]
    fn facade_build_module_exports_native_module_pipeline() {
        let root = temp_dir("pipeline");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        write_catalog(&english_dir.join("common.json"), &[("home.title", "Hi")]);
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let output = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect("build");

        assert_eq!(output.default_locale(), "en");
        assert!(output.generated_module_path().exists());
        assert!(output.generated_catalog_path().exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn facade_build_module_exports_web_json_pipeline() {
        let root = temp_dir("web_json");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        write_catalog(&english_dir.join("common.json"), &[("home.title", "Hi")]);

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let output = export_web_json(&WebJsonExportOptions::new(
            &config_path,
            root.join("web-json"),
        ))
        .expect("export");

        assert_eq!(output.default_locale(), "en");
        assert!(output.manifest_path().exists());
        assert!(output.messages_dir().join("en/common.json").exists());

        fs::remove_dir_all(&root).ok();
    }
}
