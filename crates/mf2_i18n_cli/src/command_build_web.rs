use std::path::PathBuf;

use mf2_i18n_build::{
    PlainJsonWebBuildError, PlainJsonWebBuildOptions, build_plain_json_web_target,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildWebCommandError {
    #[error(transparent)]
    Build(#[from] PlainJsonWebBuildError),
}

#[derive(Debug, Clone)]
pub struct BuildWebOptions {
    pub config_path: PathBuf,
    pub target_name: String,
    pub out_dir: Option<PathBuf>,
    pub check: bool,
}

pub fn run_build_web(options: &BuildWebOptions) -> Result<(), BuildWebCommandError> {
    let mut build_options =
        PlainJsonWebBuildOptions::new(&options.config_path, &options.target_name)
            .with_check(options.check);

    if let Some(out_dir) = &options.out_dir {
        build_options = build_options.with_out_dir(out_dir);
    }

    build_plain_json_web_target(&build_options)?;

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
}
