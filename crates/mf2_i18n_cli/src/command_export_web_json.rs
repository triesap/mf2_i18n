use std::path::PathBuf;

use mf2_i18n_build::{WebJsonExportError, WebJsonExportOptions, WebJsonMode, export_web_json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportWebJsonCommandError {
    #[error(transparent)]
    Export(#[from] WebJsonExportError),
}

#[derive(Debug, Clone)]
pub struct ExportWebJsonOptions {
    pub config_path: PathBuf,
    pub out_dir: PathBuf,
    pub mode: WebJsonMode,
}

pub fn run_export_web_json(
    options: &ExportWebJsonOptions,
) -> Result<(), ExportWebJsonCommandError> {
    export_web_json(
        &WebJsonExportOptions::new(&options.config_path, &options.out_dir).with_mode(options.mode),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ExportWebJsonOptions, run_export_web_json};
    use mf2_i18n_build::WebJsonMode;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_cli_web_json_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn exports_web_json_files() {
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
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"",
        )
        .expect("config");

        let out_dir = dir.join("web-json");
        run_export_web_json(&ExportWebJsonOptions {
            config_path,
            out_dir: out_dir.clone(),
            mode: WebJsonMode::Plain,
        })
        .expect("export");

        assert!(out_dir.join("messages/en/common.json").exists());
        assert!(out_dir.join("i18n-manifest.ts").exists());
        assert!(
            fs::read_to_string(out_dir.join("i18n-manifest.ts"))
                .expect("manifest")
                .contains("MESSAGE_LOADERS")
        );

        fs::remove_dir_all(&dir).ok();
    }
}
