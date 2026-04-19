use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_manifest(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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

fn assert_fixture_builds(name: &str) {
    let manifest_path = fixture_manifest(name);
    let output = Command::new(cargo_bin())
        .arg("check")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&manifest_path)
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

#[test]
fn supports_runtime_only_consumer_mode() {
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
