#[macro_export]
macro_rules! define_i18n_module {
    (
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
                .unwrap_or_else(|_| $crate::NativeLocalizer::fallback($default_locale))
            });

        pub fn localizer() -> &'static $crate::NativeLocalizer {
            &LOCALIZER
        }

        pub fn set_locale(locale: &str) {
            LOCALIZER.set_locale(locale);
        }

        pub fn locale() -> String {
            LOCALIZER.locale()
        }

        pub fn tr(key: &str) -> String {
            LOCALIZER.tr(key)
        }

        pub fn tr_with_args(key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args(key, args)
        }
    };
}

#[cfg(test)]
mod tests {
    mod generated {
        crate::define_i18n_module! {
            default_locale: "en",
            id_map_json: br#"{}"#,
            id_map_hash: b"sha256:00",
            packs: [("en", b"")],
        }
    }

    #[test]
    fn macro_generated_module_falls_back_to_key_on_invalid_artifacts() {
        assert_eq!(generated::locale(), "en");
        assert_eq!(generated::tr("home.title"), "home.title");
    }
}
