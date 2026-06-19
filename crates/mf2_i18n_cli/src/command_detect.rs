use std::path::PathBuf;

use serde_json::Value;

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
    detect_package_presets(&options.cwd, &mut detected_presets);
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
    detected_presets.sort();
    detected_presets.dedup();

    DetectReport {
        config_exists: options.cwd.join("mf2_i18n.toml").exists(),
        detected_presets,
    }
}

fn detect_package_presets(cwd: &PathBuf, detected_presets: &mut Vec<String>) {
    let package_path = cwd.join("package.json");
    let Ok(contents) = std::fs::read_to_string(package_path) else {
        return;
    };
    let Ok(package) = serde_json::from_str::<Value>(&contents) else {
        return;
    };

    let dependencies = ["dependencies", "devDependencies", "peerDependencies"]
        .into_iter()
        .filter_map(|field| package.get(field).and_then(Value::as_object))
        .flat_map(|object| object.keys().map(String::as_str))
        .collect::<Vec<_>>();

    if dependencies.iter().any(|name| *name == "@docusaurus/core") {
        detected_presets.push("docusaurus".to_string());
    }
    if dependencies.iter().any(|name| *name == "vite") {
        detected_presets.push("vite".to_string());
    }
    if dependencies.iter().any(|name| *name == "react") {
        detected_presets.push("react".to_string());
    }
    if dependencies.iter().any(|name| *name == "svelte") {
        detected_presets.push("svelte".to_string());
    }
    if detected_presets.is_empty() {
        detected_presets.push("vanilla".to_string());
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
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"@docusaurus/core":"latest","react":"latest","vite":"latest"}}"#,
        )
        .expect("package");
        fs::write(dir.join("docusaurus.config.ts"), "export default {};").expect("config");

        let report = run_detect(&DetectOptions { cwd: dir.clone() });
        assert_eq!(report.detected_presets, ["docusaurus", "react", "vite"]);
        assert!(!report.config_exists);

        fs::remove_dir_all(&dir).ok();
    }
}
