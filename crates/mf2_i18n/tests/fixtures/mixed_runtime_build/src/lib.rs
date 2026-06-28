include!(concat!(env!("OUT_DIR"), "/app_i18n/generated_module.rs"));

pub fn smoke() -> String {
    assert_eq!(env!("MF2_MIXED_RUNTIME_BUILD_DEFAULT_LOCALE"), DEFAULT_LOCALE);
    let _runtime: Option<&mf2_i18n::NativeRuntime> = None;
    let args = CommonHomeTitleArgs::new("Nova");
    t_common_home_title_or_key(&args)
}
