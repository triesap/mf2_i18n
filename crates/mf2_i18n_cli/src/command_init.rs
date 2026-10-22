use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InitCommandError {
    #[error("config already exists: {0}")]
    ConfigExists(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("package script argument {name} contains unsupported characters: {value}")]
    PackageScriptArg { name: &'static str, value: String },
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub config_path: PathBuf,
    pub preset: String,
    pub source_layout: String,
    pub target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub config_path: PathBuf,
}

pub fn run_init(options: &InitOptions) -> Result<InitReport, InitCommandError> {
    if options.config_path.exists() {
        return Err(InitCommandError::ConfigExists(
            options.config_path.display().to_string(),
        ));
    }
    let config_root = options
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let package_root = find_package_root(config_root);
    if let Some(package_root) = package_root.as_deref() {
        validate_package_script_args(package_root, options)?;
    }
    if let Some(parent) = options.config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&options.config_path, render_config(options))?;
    write_starter_locale(config_root, options)?;
    write_project_salt(config_root)?;
    write_generated_ignore(config_root, options)?;
    write_leptos_build_script(config_root, options)?;
    if let Some(package_root) = package_root.as_deref() {
        update_package_scripts(package_root, options)?;
    }
    Ok(InitReport {
        config_path: options.config_path.clone(),
    })
}

fn render_config(options: &InitOptions) -> String {
    if is_leptos_preset(&options.preset) {
        return render_leptos_config(options);
    }

    format!(
        "version = 1\n\
default_locale = \"en\"\n\
locales = [\"en\"]\n\n\
[[sources]]\n\
name = \"site\"\n\
layout = {source_layout:?}\n\
root = \"locales\"\n\
namespace_glob = \"*.mf2\"\n\n\
[[targets]]\n\
name = {target_name:?}\n\
source = \"site\"\n\
preset = {preset:?}\n\
out_dir = \"src/generated/mf2\"\n\
typescript = true\n",
        source_layout = options.source_layout,
        target_name = options.target_name,
        preset = options.preset
    )
}

fn render_leptos_config(options: &InitOptions) -> String {
    let native_target_name = native_target_name(options);
    format!(
        "version = 1\n\
default_locale = \"en\"\n\
locales = [\"en\"]\n\n\
[[sources]]\n\
name = \"site\"\n\
layout = {source_layout:?}\n\
root = \"locales\"\n\
namespace_glob = \"*.mf2\"\n\n\
[[native.targets]]\n\
name = {native_target_name:?}\n\
out_dir = \"target/mf2/native\"\n\
module_name = \"app_i18n\"\n\n\
[[leptos.targets]]\n\
name = {target_name:?}\n\
native_target = {native_target_name:?}\n\
out_dir = \"src/generated/i18n\"\n\
module_name = \"app_i18n\"\n\
sync_html_lang = true\n",
        source_layout = options.source_layout,
        target_name = options.target_name,
        native_target_name = native_target_name
    )
}

fn write_starter_locale(
    project_root: &Path,
    options: &InitOptions,
) -> Result<(), InitCommandError> {
    match options.source_layout.as_str() {
        "locale-namespace" | "docusaurus-docs" => {
            let path = project_root.join("locales/en/app.mf2");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            if !path.exists() {
                fs::write(path, "title = TODO\n")?;
            }
        }
        "namespace-locale" => {
            let path = project_root.join("locales/app/en.mf2");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            if !path.exists() {
                fs::write(path, "title = TODO\n")?;
            }
        }
        "single-locale-file" => {
            let path = project_root.join("locales/en.mf2");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            if !path.exists() {
                fs::write(path, "app.title = TODO\n")?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn write_project_salt(project_root: &Path) -> Result<(), InitCommandError> {
    let salt_path = project_root.join("id_salt.txt");
    if !salt_path.exists() {
        fs::write(salt_path, "change-me-project-salt\n")?;
    }
    Ok(())
}

fn write_generated_ignore(
    project_root: &Path,
    options: &InitOptions,
) -> Result<(), InitCommandError> {
    let generated_dir = if is_leptos_preset(&options.preset) {
        project_root.join("src/generated/i18n")
    } else {
        project_root.join("src/generated/mf2")
    };
    fs::create_dir_all(&generated_dir)?;
    fs::write(generated_dir.join(".gitignore"), "*\n!.gitignore\n")?;
    Ok(())
}

fn write_leptos_build_script(
    project_root: &Path,
    options: &InitOptions,
) -> Result<(), InitCommandError> {
    if !is_leptos_preset(&options.preset) {
        return Ok(());
    }
    let build_script_path = project_root.join("build.rs");
    if build_script_path.exists() {
        return Ok(());
    }
    let config_path = package_script_config_path(project_root, &options.config_path);
    fs::write(
        build_script_path,
        format!(
            "fn main() {{\n    let manifest_dir =\n        std::path::PathBuf::from(std::env::var(\"CARGO_MANIFEST_DIR\").expect(\"manifest dir\"));\n    let out_dir = std::path::PathBuf::from(std::env::var(\"OUT_DIR\").expect(\"out dir\"));\n    let config_path = manifest_dir.join({config_path:?});\n    let output = mf2_i18n::build_leptos_target(\n        &mf2_i18n::LeptosTargetBuildOptions::new(&config_path, {target_name:?})\n            .with_out_dir(&out_dir),\n    )\n    .expect(\"leptos target\");\n\n    for path in output.native_output().rerun_if_changed_paths() {{\n        println!(\"cargo:rerun-if-changed={{}}\", path.display());\n    }}\n}}\n",
            target_name = options.target_name
        ),
    )?;
    Ok(())
}

fn find_package_root(config_root: &Path) -> Option<PathBuf> {
    for ancestor in config_root.ancestors() {
        if ancestor.join("package.json").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn update_package_scripts(
    project_root: &Path,
    options: &InitOptions,
) -> Result<(), InitCommandError> {
    let package_path = project_root.join("package.json");
    if !package_path.exists() {
        return Ok(());
    }
    let config_path = package_script_config_path(project_root, &options.config_path);

    let contents = fs::read_to_string(&package_path)?;
    let mut package: Value = serde_json::from_str(&contents)?;
    let object = package.as_object_mut().ok_or_else(|| {
        serde_json::Error::io(std::io::Error::other("package root is not object"))
    })?;
    let scripts = object
        .entry("scripts")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let scripts = scripts
        .as_object_mut()
        .ok_or_else(|| serde_json::Error::io(std::io::Error::other("scripts is not object")))?;
    scripts.insert(
        "mf2:gen".to_string(),
        Value::String(mf2_gen_script(config_path.as_str(), options)),
    );
    if !is_leptos_preset(&options.preset) {
        scripts.insert(
            "mf2:check".to_string(),
            Value::String(format!(
                "mf2-i18n check-web --config {config_path} --target {}",
                options.target_name,
            )),
        );
    }

    let mut encoded = serde_json::to_string_pretty(&package)?;
    encoded.push('\n');
    fs::write(package_path, encoded)?;
    Ok(())
}

fn mf2_gen_script(config_path: &str, options: &InitOptions) -> String {
    if is_leptos_preset(&options.preset) {
        format!(
            "mf2-i18n build-leptos --config {config_path} --target {}",
            options.target_name
        )
    } else {
        format!(
            "mf2-i18n build-web --config {config_path} --target {}",
            options.target_name
        )
    }
}

fn native_target_name(options: &InitOptions) -> String {
    format!("{}-native", options.target_name)
}

fn is_leptos_preset(preset: &str) -> bool {
    preset == "leptos"
}

fn validate_package_script_args(
    project_root: &Path,
    options: &InitOptions,
) -> Result<(), InitCommandError> {
    validate_script_arg("target", &options.target_name, false)?;
    validate_script_arg(
        "config",
        &package_script_config_path(project_root, &options.config_path),
        true,
    )?;
    Ok(())
}

fn validate_script_arg(
    name: &'static str,
    value: &str,
    allow_slash: bool,
) -> Result<(), InitCommandError> {
    let is_safe = !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '_' | '-' | '.')
                || (allow_slash && ch == '/')
        });
    if is_safe {
        Ok(())
    } else {
        Err(InitCommandError::PackageScriptArg {
            name,
            value: value.to_string(),
        })
    }
}

fn package_script_config_path(project_root: &Path, config_path: &Path) -> String {
    let path = if config_path.is_absolute() {
        config_path
            .strip_prefix(project_root)
            .unwrap_or(config_path)
    } else {
        config_path
    };
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{InitCommandError, InitOptions, run_init};
    use crate::command_build_web::{BuildWebOptions, run_build_web};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    const LEPTOS_ALPHA_COMPATIBILITY_DEPENDENCIES: &str = r#"
hydration_context = "=0.3.1"
leptos = { version = "=0.9.0-alpha", default-features = false }
leptos_config = "=0.9.0-alpha"
leptos_dom = "=0.9.0-alpha"
leptos_hot_reload = "=0.9.0-alpha"
leptos_macro = "=0.9.0-alpha"
leptos_server = "=0.9.0-alpha"
reactive_graph = "=0.3.0-alpha"
reactive_stores = "=0.5.0-alpha"
server_fn = "=0.9.0-alpha"
server_fn_macro = "=0.9.0-alpha"
server_fn_macro_default = "=0.9.0-alpha"
tachys = "=0.3.0-alpha"
throw_error = "=0.3.1"
"#;

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!("mf2_i18n_init_{nanos}_{counter}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn facade_crate_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .join("mf2_i18n")
    }

    fn assert_manifest_tests(name: &str, manifest_path: &Path) {
        let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .arg("test")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--target-dir")
            .arg(
                manifest_path
                    .parent()
                    .expect("manifest parent")
                    .join("target"),
            )
            .output()
            .expect("fixture cargo test should run");

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("fixture `{name}` failed cargo test\nstdout:\n{stdout}\nstderr:\n{stderr}");
        }
    }

    #[test]
    fn writes_v1_config() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path: config_path.clone(),
            preset: "vite".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "web".to_string(),
        })
        .expect("init");

        let contents = fs::read_to_string(config_path).expect("read");
        assert!(contents.contains("version = 1"));
        assert!(contents.contains("preset = \"vite\""));
        assert!(dir.join("locales/en/app.mf2").exists());
        assert!(dir.join("id_salt.txt").exists());
        assert!(dir.join("src/generated/mf2/.gitignore").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adds_package_scripts_when_package_json_exists() {
        let dir = temp_dir();
        fs::write(dir.join("package.json"), "{}").expect("package");
        let config_dir = dir.join("i18n");
        let config_path = config_dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path,
            preset: "react".to_string(),
            source_layout: "single-locale-file".to_string(),
            target_name: "app".to_string(),
        })
        .expect("init");

        let package = fs::read_to_string(dir.join("package.json")).expect("read");
        assert!(package.contains(
            "\"mf2:gen\": \"mf2-i18n build-web --config i18n/mf2_i18n.toml --target app\""
        ));
        assert!(package.contains(
            "\"mf2:check\": \"mf2-i18n check-web --config i18n/mf2_i18n.toml --target app\""
        ));
        assert!(config_dir.join("locales/en.mf2").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writes_leptos_config_without_web_targets() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path: config_path.clone(),
            preset: "leptos".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "app".to_string(),
        })
        .expect("init");

        let contents = fs::read_to_string(config_path).expect("read");
        assert!(contents.contains("[[native.targets]]"));
        assert!(contents.contains("name = \"app-native\""));
        assert!(contents.contains("[[leptos.targets]]"));
        assert!(contents.contains("native_target = \"app-native\""));
        assert!(!contents.contains("[[targets]]"));
        assert!(!contents.contains("typescript = true"));
        assert!(dir.join("locales/en/app.mf2").exists());
        assert!(dir.join("src/generated/i18n/.gitignore").exists());
        assert!(dir.join("build.rs").exists());
        assert!(!dir.join("src/generated/mf2/.gitignore").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn leptos_package_script_uses_leptos_build_target() {
        let dir = temp_dir();
        fs::write(dir.join("package.json"), "{}").expect("package");
        let config_path = dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path,
            preset: "leptos".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "app".to_string(),
        })
        .expect("init");

        let package = fs::read_to_string(dir.join("package.json")).expect("read");
        assert!(package.contains(
            "\"mf2:gen\": \"mf2-i18n build-leptos --config mf2_i18n.toml --target app\""
        ));
        assert!(!package.contains("mf2:check"));
        assert!(!package.contains("build-web"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn initialized_leptos_project_builds_from_cargo_out_dir() {
        let dir = temp_dir();
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                r#"[package]
name = "mf2_i18n_initialized_leptos_fixture"
version = "0.0.0"
edition = "2024"

[workspace]

[dependencies]
mf2_i18n = {{ path = {path:?}, default-features = false, features = ["leptos"] }}
{compatibility_dependencies}

[build-dependencies]
mf2_i18n = {{ path = {path:?}, default-features = false, features = ["build"] }}
"#,
                path = facade_crate_path().to_string_lossy().replace('\\', "/"),
                compatibility_dependencies = LEPTOS_ALPHA_COMPATIBILITY_DEPENDENCIES,
            ),
        )
        .expect("manifest");
        fs::create_dir_all(dir.join("src")).expect("src");
        fs::write(
            dir.join("src/lib.rs"),
            r#"include!(concat!(env!("OUT_DIR"), "/app_i18n.rs"));

#[cfg(test)]
mod tests {
    use leptos::prelude::Owner;

    use super::{MessageKey, localizer, t_app_title, text_app_title};

    #[test]
    fn generated_scaffold_formats_starter_text() {
        assert_eq!(t_app_title(), "TODO");
        Owner::new().with(|| {
            let context = mf2_i18n::leptos::provide_i18n(localizer());
            assert_eq!(text_app_title(&context), "TODO");
            assert_eq!(context.text(MessageKey::AppTitle), "TODO");
        });
    }
}
"#,
        )
        .expect("source");
        let config_path = dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path: config_path.clone(),
            preset: "leptos".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "app".to_string(),
        })
        .expect("init");

        let config = fs::read_to_string(&config_path).expect("config");
        assert!(config.contains("[[native.targets]]"));
        assert!(config.contains("[[leptos.targets]]"));
        assert!(config.contains("out_dir = \"src/generated/i18n\""));
        let build_script = fs::read_to_string(dir.join("build.rs")).expect("build script");
        assert!(build_script.contains("std::env::var(\"OUT_DIR\")"));
        assert!(build_script.contains("mf2_i18n::build_leptos_target"));
        assert!(build_script.contains(".with_out_dir(&out_dir)"));
        assert!(dir.join("locales/en/app.mf2").exists());
        assert!(dir.join("src/generated/i18n/.gitignore").exists());

        assert_manifest_tests("initialized_leptos", &dir.join("Cargo.toml"));

        assert!(!dir.join("src/generated/i18n/app_i18n.rs").exists());
        assert!(!dir.join("target/mf2/native/generated_module.rs").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn initialized_project_builds_web_target() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        run_init(&InitOptions {
            config_path: config_path.clone(),
            preset: "vite".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "web".to_string(),
        })
        .expect("init");

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "web".to_string(),
            out_dir: Some(dir.join("out")),
            check: false,
            dto_types: false,
        })
        .expect("build-web");

        assert!(dir.join("out/mf2-web-manifest.json").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_existing_config() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(&config_path, "").expect("write");

        let err = run_init(&InitOptions {
            config_path,
            preset: "vite".to_string(),
            source_layout: "locale-namespace".to_string(),
            target_name: "web".to_string(),
        })
        .expect_err("exists");

        assert!(matches!(err, InitCommandError::ConfigExists(_)));

        fs::remove_dir_all(&dir).ok();
    }
}
