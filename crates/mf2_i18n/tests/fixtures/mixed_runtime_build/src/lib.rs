mod generated {
    mf2_i18n::define_i18n_module! {
        init_policy: fallback_to_keys,
        default_locale: "en",
        id_map_json: br#"{}"#,
        id_map_hash: b"sha256:00",
        packs: [("en", b"")],
    }
}

pub fn smoke() -> String {
    let _default_locale = env!("MF2_MIXED_RUNTIME_BUILD_DEFAULT_LOCALE");
    let _runtime: Option<&mf2_i18n::NativeRuntime> = None;
    generated::locale()
}
