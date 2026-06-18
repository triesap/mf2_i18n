use std::path::PathBuf;

use mf2_i18n_build::ProjectLayout;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DoctorCommandError {
    #[error(transparent)]
    Project(#[from] mf2_i18n_build::ProjectError),
}

#[derive(Debug, Clone)]
pub struct DoctorOptions {
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub default_locale: String,
    pub source_count: usize,
    pub target_count: usize,
    pub web_target_count: usize,
}

pub fn run_doctor(options: &DoctorOptions) -> Result<DoctorReport, DoctorCommandError> {
    let project = ProjectLayout::load(&options.config_path)?;
    Ok(DoctorReport {
        default_locale: project.config().default_locale.clone(),
        source_count: project.config().sources.len(),
        target_count: project.config().targets.len(),
        web_target_count: project.config().web.targets.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::{DoctorOptions, run_doctor};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_doctor_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn reports_config_shape() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
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
source = "site"
preset = "vite"
out_dir = "src/generated/mf2"
"#,
        )
        .expect("config");

        let report = run_doctor(&DoctorOptions { config_path }).expect("doctor");
        assert_eq!(report.default_locale, "en");
        assert_eq!(report.source_count, 1);
        assert_eq!(report.target_count, 1);
        assert_eq!(report.web_target_count, 1);

        fs::remove_dir_all(&dir).ok();
    }
}
