use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn facade_crate_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn fixture_manifest(name: &str) -> PathBuf {
    facade_crate_path()
        .join("tests")
        .join("fixtures")
        .join(name)
        .join("Cargo.toml")
}

fn unique_target_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    path.push(format!("mf2_i18n_consumer_mode_{name}_{nanos}"));
    path
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn assert_manifest_builds(name: &str, manifest_path: &Path) {
    let output = Command::new(cargo_bin())
        .arg("check")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target-dir")
        .arg(unique_target_dir(name))
        .output()
        .expect("fixture cargo check should run");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("fixture `{name}` failed to build\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }
}

fn assert_fixture_builds(name: &str) {
    assert_manifest_builds(name, &fixture_manifest(name));
}

fn assert_inline_fixture_builds(name: &str, features: &str, source: &str) {
    let fixture_dir = unique_target_dir(&format!("{name}_fixture"));
    let src_dir = fixture_dir.join("src");
    fs::create_dir_all(&src_dir).expect("src");
    fs::write(
        fixture_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "mf2_i18n_facade_{name}_fixture"
version = "0.0.0"
edition = "2024"

[workspace]

[dependencies]
mf2_i18n = {{ path = {path:?}, default-features = false, features = {features} }}
"#,
            path = facade_crate_path().to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("manifest");
    fs::write(src_dir.join("lib.rs"), source).expect("source");
    assert_manifest_builds(name, &fixture_dir.join("Cargo.toml"));
}

#[test]
fn supports_core_consumer_mode() {
    assert_inline_fixture_builds(
        "core_only",
        "[]",
        r#"
pub fn smoke() -> mf2_i18n::Args {
    mf2_i18n::Args::new()
}
"#,
    );
}

#[test]
fn supports_runtime_only_consumer_mode() {
    assert_inline_fixture_builds(
        "runtime_only",
        r#"["runtime"]"#,
        r#"
pub fn smoke() {
    let _runtime: Option<mf2_i18n::Runtime> = None;
    let _manifest: Option<mf2_i18n::Manifest> = None;
}
"#,
    );
}

#[test]
fn supports_native_consumer_mode() {
    assert_fixture_builds("runtime_only");
}

#[test]
fn supports_build_only_consumer_mode() {
    assert_fixture_builds("build_only");
}

#[test]
fn supports_mixed_runtime_and_build_consumer_mode() {
    assert_fixture_builds("mixed_runtime_build");
}

#[test]
fn supports_leptos_consumer_mode() {
    assert_inline_fixture_builds(
        "leptos",
        r#"["leptos"]"#,
        r#"
#[derive(Clone, Copy)]
enum Key {
    HomeTitle,
}

impl mf2_i18n::leptos::I18nMessageKey for Key {
    fn id(self) -> &'static str {
        match self {
            Self::HomeTitle => "home.title",
        }
    }
}

pub fn smoke(context: mf2_i18n::leptos::I18nContext) -> String {
    context.text_or_key(Key::HomeTitle)
}
"#,
    );
}
