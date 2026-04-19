#[cfg(feature = "build")]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use mf2_i18n::{
        BuildIoError, CompileError, PlatformBundleManifest, ProjectConfig, ProjectLayout,
        load_project_config_or_default, resolve_config_relative_path,
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

    #[test]
    fn root_exports_build_surface() {
        let path = temp_path("missing");
        let config = load_project_config_or_default(&path).expect("config");
        assert_eq!(config.default_locale, "en");

        let _compile_error: Option<CompileError> = None;
        let _io_error: Option<BuildIoError> = None;
        let _layout: Option<ProjectLayout> = None;
        let _manifest: Option<PlatformBundleManifest> = None;
        let project_root = resolve_config_relative_path(&path, "locales");
        assert!(project_root.ends_with("locales"));

        let default_config = ProjectConfig::default();
        assert_eq!(default_config.project_salt_path, "tools/id_salt.txt");
    }
}
