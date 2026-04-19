#[macro_export]
macro_rules! define_i18n_module {
    (
        init_policy: strict,
        default_locale: $default_locale:expr,
        id_map_json: $id_map_json:expr,
        id_map_hash: $id_map_hash:expr,
        packs: [$(($locale:expr, $pack_bytes:expr)),+ $(,)?],
    ) => {
        static LOCALIZER: ::std::sync::LazyLock<$crate::NativeLocalizer> =
            ::std::sync::LazyLock::new(|| {
                let packs = [
                    $(
                        $crate::EmbeddedPack {
                            locale: $locale,
                            bytes: $pack_bytes,
                        },
                    )+
                ];
                $crate::NativeLocalizer::from_embedded_artifacts(
                    $default_locale,
                    $id_map_json,
                    $id_map_hash,
                    &packs,
                )
                .expect("mf2-i18n native module failed to initialize")
            });

        pub fn localizer() -> &'static $crate::NativeLocalizer {
            &LOCALIZER
        }

        pub fn set_preferred_locales(preferred_locales: &[&str]) -> $crate::NativeResult<String> {
            LOCALIZER.set_preferred_locales(preferred_locales.iter().copied())
        }

        pub fn set_locale(locale: &str) -> $crate::NativeResult<String> {
            LOCALIZER.set_locale(locale)
        }

        pub fn locale() -> String {
            LOCALIZER.locale()
        }

        pub fn default_locale() -> String {
            LOCALIZER.default_locale()
        }

        pub fn preferred_locales() -> ::std::vec::Vec<String> {
            LOCALIZER.preferred_locales()
        }

        pub fn supported_locales() -> ::std::vec::Vec<String> {
            LOCALIZER.supported_locales()
        }

        pub fn tr(key: &str) -> $crate::NativeResult<String> {
            LOCALIZER.tr(key)
        }

        pub fn tr_with_args(key: &str, args: &$crate::Args) -> $crate::NativeResult<String> {
            LOCALIZER.tr_with_args(key, args)
        }

        pub fn tr_or_key(key: &str) -> String {
            LOCALIZER.tr_or_key(key)
        }

        pub fn tr_with_args_or_key(key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args_or_key(key, args)
        }

        pub fn format(key: &str, args: &$crate::Args) -> $crate::NativeResult<String> {
            LOCALIZER.format(key, args)
        }
    };
    (
        init_policy: fallback_to_keys,
        default_locale: $default_locale:expr,
        id_map_json: $id_map_json:expr,
        id_map_hash: $id_map_hash:expr,
        packs: [$(($locale:expr, $pack_bytes:expr)),+ $(,)?],
    ) => {
        static LOCALIZER: ::std::sync::LazyLock<$crate::NativeLocalizer> =
            ::std::sync::LazyLock::new(|| {
                let packs = [
                    $(
                        $crate::EmbeddedPack {
                            locale: $locale,
                            bytes: $pack_bytes,
                        },
                    )+
                ];
                $crate::NativeLocalizer::from_embedded_artifacts_or_fallback(
                    $default_locale,
                    $id_map_json,
                    $id_map_hash,
                    &packs,
                )
            });

        pub fn localizer() -> &'static $crate::NativeLocalizer {
            &LOCALIZER
        }

        pub fn set_preferred_locales(preferred_locales: &[&str]) -> $crate::NativeResult<String> {
            LOCALIZER.set_preferred_locales(preferred_locales.iter().copied())
        }

        pub fn set_locale(locale: &str) -> $crate::NativeResult<String> {
            LOCALIZER.set_locale(locale)
        }

        pub fn locale() -> String {
            LOCALIZER.locale()
        }

        pub fn default_locale() -> String {
            LOCALIZER.default_locale()
        }

        pub fn preferred_locales() -> ::std::vec::Vec<String> {
            LOCALIZER.preferred_locales()
        }

        pub fn supported_locales() -> ::std::vec::Vec<String> {
            LOCALIZER.supported_locales()
        }

        pub fn tr(key: &str) -> $crate::NativeResult<String> {
            LOCALIZER.tr(key)
        }

        pub fn tr_with_args(key: &str, args: &$crate::Args) -> $crate::NativeResult<String> {
            LOCALIZER.tr_with_args(key, args)
        }

        pub fn tr_or_key(key: &str) -> String {
            LOCALIZER.tr_or_key(key)
        }

        pub fn tr_with_args_or_key(key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args_or_key(key, args)
        }

        pub fn format(key: &str, args: &$crate::Args) -> $crate::NativeResult<String> {
            LOCALIZER.format(key, args)
        }
    };
}

#[cfg(test)]
mod tests {
    mod generated_fallback {
        crate::define_i18n_module! {
            init_policy: fallback_to_keys,
            default_locale: "en",
            id_map_json: br#"{}"#,
            id_map_hash: b"sha256:00",
            packs: [("en", b"")],
        }
    }

    mod generated_strict {
        crate::define_i18n_module! {
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
            Err(crate::NativeError::NotInitialized)
        ));
        assert_eq!(generated_fallback::tr_or_key("home.title"), "home.title");
    }

    #[test]
    fn macro_generated_module_uses_explicit_strict_init_policy() {
        let result = std::panic::catch_unwind(generated_strict::localizer);
        assert!(result.is_err());
    }
}
