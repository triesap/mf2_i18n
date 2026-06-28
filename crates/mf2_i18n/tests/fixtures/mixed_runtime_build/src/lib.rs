include!(concat!(env!("OUT_DIR"), "/app_i18n/generated_module.rs"));

pub fn smoke() -> String {
    assert_eq!(env!("MF2_MIXED_RUNTIME_BUILD_DEFAULT_LOCALE"), DEFAULT_LOCALE);
    let _runtime: Option<&mf2_i18n::NativeRuntime> = None;
    let mut args = mf2_i18n::Args::new();
    args.insert("name", mf2_i18n::Value::Str("Nova".to_string()));
    t_with_args_or_key(MessageKey::CommonHomeTitle, &args)
}
