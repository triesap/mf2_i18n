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
    let _localizer: Option<&mf2_i18n::NativeLocalizer> = None;
    let _pack = mf2_i18n::EmbeddedPack {
        locale: "en",
        bytes: b"",
    };

    let mut args = mf2_i18n::Args::new();
    args.insert("count", mf2_i18n::Value::Num(1.0));
    generated::tr_with_args_or_key("home.title", &args)
}
