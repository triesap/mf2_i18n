use std::path::PathBuf;

use mf2_i18n_build::{
    PlainJsonWebBuildError, PlainJsonWebBuildOptions, ProjectError, ProjectLayout,
    RuntimeWebBuildError, RuntimeWebBuildOptions, WebTargetConfigError, WebTargetMode,
    build_plain_json_web_target, build_runtime_web_target,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildWebCommandError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] WebTargetConfigError),
    #[error(transparent)]
    PlainJson(#[from] PlainJsonWebBuildError),
    #[error(transparent)]
    Runtime(#[from] RuntimeWebBuildError),
    #[error("missing web target {0}")]
    MissingTarget(String),
    #[error("web target {target} mode is not supported by build-web yet")]
    UnsupportedTargetMode { target: String },
}

#[derive(Debug, Clone)]
pub struct BuildWebOptions {
    pub config_path: PathBuf,
    pub target_name: String,
    pub out_dir: Option<PathBuf>,
    pub check: bool,
}

pub fn run_build_web(options: &BuildWebOptions) -> Result<(), BuildWebCommandError> {
    let project = ProjectLayout::load_or_default(&options.config_path)?;
    project.config().validate_web_targets()?;
    let target = project
        .config()
        .web
        .target(&options.target_name)
        .ok_or_else(|| BuildWebCommandError::MissingTarget(options.target_name.clone()))?;

    match target.mode {
        WebTargetMode::PlainJson => {
            let mut build_options =
                PlainJsonWebBuildOptions::new(&options.config_path, &options.target_name)
                    .with_check(options.check);

            if let Some(out_dir) = &options.out_dir {
                build_options = build_options.with_out_dir(out_dir);
            }

            build_plain_json_web_target(&build_options)?;
        }
        WebTargetMode::Runtime => {
            let mut build_options =
                RuntimeWebBuildOptions::new(&options.config_path, &options.target_name)
                    .with_check(options.check);

            if let Some(out_dir) = &options.out_dir {
                build_options = build_options.with_out_dir(out_dir);
            }

            build_runtime_web_target(&build_options)?;
        }
        WebTargetMode::Docusaurus | WebTargetMode::Hybrid => {
            return Err(BuildWebCommandError::UnsupportedTargetMode {
                target: target.name.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BuildWebOptions, run_build_web};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_cli_build_web_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn builds_named_web_target() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "plain-json"
out_dir = "web-generated"
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path: config_path.clone(),
            target_name: "docs".to_string(),
            out_dir: None,
            check: false,
        })
        .expect("build");

        let out_dir = dir.join("web-generated");
        assert!(out_dir.join("mf2-web-manifest.json").exists());
        assert!(out_dir.join("types.ts").exists());
        assert!(out_dir.join("messages/en/common.json").exists());

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "docs".to_string(),
            out_dir: Some(out_dir),
            check: true,
        })
        .expect("check");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builds_runtime_web_target() {
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
            r#"
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "runtime"
mode = "runtime"
out_dir = "web-runtime"
release_id = "test-runtime"
generated_at = "2026-02-01T00:00:00Z"
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "runtime".to_string(),
            out_dir: None,
            check: false,
        })
        .expect("build");

        let out_dir = dir.join("web-runtime");
        assert!(out_dir.join("runtime-assets.json").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());
        assert!(out_dir.join("loaders.ts").exists());

        fs::remove_dir_all(&dir).ok();
    }
}
