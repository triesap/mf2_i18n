fn main() {
    let config = mf2_i18n::ProjectConfig::default();
    let _layout: Option<mf2_i18n::ProjectLayout> = None;
    let _manifest: Option<mf2_i18n::PlatformBundleManifest> = None;

    println!("cargo:rustc-env=MF2_BUILD_ONLY_DEFAULT_LOCALE={}", config.default_locale);
}
