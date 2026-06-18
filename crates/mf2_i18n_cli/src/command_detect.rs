use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DetectOptions {
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectReport {
    pub config_exists: bool,
    pub detected_presets: Vec<String>,
}

pub fn run_detect(options: &DetectOptions) -> DetectReport {
    let mut detected_presets = Vec::new();
    if options.cwd.join("package.json").exists() {
        detected_presets.push("vanilla".to_string());
    }
    if [
        "docusaurus.config.js",
        "docusaurus.config.ts",
        "docusaurus.config.mjs",
    ]
    .iter()
    .any(|path| options.cwd.join(path).exists())
    {
        detected_presets.push("docusaurus".to_string());
    }

    DetectReport {
        config_exists: options.cwd.join("mf2_i18n.toml").exists(),
        detected_presets,
    }
}

#[cfg(test)]
mod tests {
    use super::{DetectOptions, run_detect};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_detect_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn detects_package_and_docusaurus_projects() {
        let dir = temp_dir();
        fs::write(dir.join("package.json"), "{}").expect("package");
        fs::write(dir.join("docusaurus.config.ts"), "export default {};").expect("config");

        let report = run_detect(&DetectOptions { cwd: dir.clone() });
        assert_eq!(report.detected_presets, ["vanilla", "docusaurus"]);
        assert!(!report.config_exists);

        fs::remove_dir_all(&dir).ok();
    }
}
