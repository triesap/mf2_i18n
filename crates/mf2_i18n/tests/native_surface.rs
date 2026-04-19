#[cfg(feature = "native")]
mod tests {
    use mf2_i18n::{Args, EmbeddedPack, NativeLocalizer, NativeRuntime, Value};

    mod generated {
        mf2_i18n::define_i18n_module! {
            init_policy: fallback_to_keys,
            default_locale: "en",
            id_map_json: br#"{}"#,
            id_map_hash: b"sha256:00",
            packs: [("en", b"")],
        }
    }

    #[test]
    fn root_exports_native_surface() {
        let _localizer: Option<NativeLocalizer> = None;
        let _runtime: Option<NativeRuntime> = None;
        let _pack = EmbeddedPack {
            locale: "en",
            bytes: b"",
        };

        let mut args = Args::new();
        args.insert("count", Value::Num(2.0));

        assert_eq!(generated::default_locale(), "en");
        assert_eq!(
            generated::tr_with_args_or_key("home.title", &args),
            "home.title"
        );
    }
}
