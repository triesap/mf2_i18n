fn main() {
    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let config_path = manifest_dir.join("mf2_i18n.toml");
    let output = mf2_i18n::build_native_module(&mf2_i18n::NativeModuleBuildOptions::new(
        &config_path,
        &out_dir,
        "app_i18n",
    ))
    .expect("native module");

    for path in output.rerun_if_changed_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!(
        "cargo:rustc-env=MF2_MIXED_RUNTIME_BUILD_DEFAULT_LOCALE={}",
        output.default_locale()
    );
}
