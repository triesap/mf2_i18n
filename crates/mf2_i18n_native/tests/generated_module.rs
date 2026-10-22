#![forbid(unsafe_code)]

pub mod generated_fallback {
    mf2_i18n_native::define_i18n_module! {
        init_policy: fallback_to_keys,
        default_locale: "en",
        id_map_json: br#"{}"#,
        id_map_hash: b"sha256:00",
        packs: [("en", b"")],
    }
}

pub mod generated_strict {
    mf2_i18n_native::define_i18n_module! {
        init_policy: strict,
        default_locale: "en",
        id_map_json: br#"{}"#,
        id_map_hash: b"sha256:00",
        packs: [("en", b"")],
    }
}

#[test]
fn macro_generated_module_falls_back_to_key_on_invalid_artifacts() {
    assert_eq!(generated_fallback::locale(), "en");
    assert_eq!(generated_fallback::default_locale(), "en");
    assert_eq!(
        generated_fallback::supported_locales(),
        vec!["en".to_string()]
    );
    assert!(matches!(
        generated_fallback::tr("home.title"),
        Err(mf2_i18n_native::NativeError::NotInitialized)
    ));
    assert_eq!(
        generated_fallback::negotiate_locale(&["en-US"]).expect("locale"),
        "en-US"
    );
    assert_eq!(generated_fallback::tr_or_key("home.title"), "home.title");
    assert_eq!(
        generated_fallback::tr_locale_or_key("en-US", "home.title"),
        "home.title"
    );
}

#[test]
fn macro_generated_module_uses_explicit_strict_init_policy() {
    let result = std::panic::catch_unwind(generated_strict::localizer);
    assert!(result.is_err());
}
