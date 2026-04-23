use std::path::PathBuf;

use mf2_i18n_build::{
    NativeModuleBuildError, ProjectRuntimeBuildOptions, build_project_runtime_artifacts,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildCommandError {
    #[error(transparent)]
    Build(#[from] NativeModuleBuildError),
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub config_path: PathBuf,
    pub out_dir: PathBuf,
    pub release_id: String,
    pub generated_at: String,
}

pub fn run_build(options: &BuildOptions) -> Result<(), BuildCommandError> {
    build_project_runtime_artifacts(&ProjectRuntimeBuildOptions::new(
        &options.config_path,
        &options.out_dir,
        options.release_id.as_str(),
        options.generated_at.as_str(),
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BuildCommandError, BuildOptions, run_build};
    use mf2_i18n_build::NativeModuleBuildError;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_build_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn builds_manifest_and_pack() {
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
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"",
        )
        .expect("config");

        let out_dir = dir.join("out");
        run_build(&BuildOptions {
            config_path,
            out_dir: out_dir.clone(),
            release_id: "r1".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
        })
        .expect("build");

        assert!(out_dir.join("manifest.json").exists());
        assert!(out_dir.join("id-map.json").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());
        assert!(out_dir.join("platform-bundle.json").exists());
        assert!(
            fs::read_to_string(out_dir.join("id-map.json"))
                .expect("id map")
                .contains("\"common.home.title\"")
        );

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
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"",
        )
        .expect("config");

        let err = run_build(&BuildOptions {
            config_path,
            out_dir: dir.join("out"),
            release_id: "r1".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
        })
        .expect_err("build should fail");

        assert!(matches!(
            err,
            BuildCommandError::Build(NativeModuleBuildError::Compile { key, .. })
                if key == "common.home.title"
        ));

        fs::remove_dir_all(&dir).ok();
    }
}
