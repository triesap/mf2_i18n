fn main() {
    let _layout: Option<mf2_i18n::ProjectLayout> = None;
    let config = mf2_i18n::ProjectConfig::default();

    println!(
        "cargo:rustc-env=MF2_MIXED_RUNTIME_BUILD_DEFAULT_LOCALE={}",
        config.default_locale
    );
}
