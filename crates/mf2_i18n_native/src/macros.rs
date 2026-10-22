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
                .expect("mf2_i18n native module failed to initialize")
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

        pub fn negotiate_locale(preferred_locales: &[&str]) -> $crate::NativeResult<String> {
            LOCALIZER.negotiate_locale(preferred_locales.iter().copied())
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

        pub fn tr_locale(locale: &str, key: &str) -> $crate::NativeResult<String> {
            LOCALIZER.tr_locale(locale, key)
        }

        pub fn tr_with_args_locale(
            locale: &str,
            key: &str,
            args: &$crate::Args,
        ) -> $crate::NativeResult<String> {
            LOCALIZER.tr_with_args_locale(locale, key, args)
        }

        pub fn tr_or_key(key: &str) -> String {
            LOCALIZER.tr_or_key(key)
        }

        pub fn tr_with_args_or_key(key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args_or_key(key, args)
        }

        pub fn tr_locale_or_key(locale: &str, key: &str) -> String {
            LOCALIZER.tr_locale_or_key(locale, key)
        }

        pub fn tr_with_args_locale_or_key(locale: &str, key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args_locale_or_key(locale, key, args)
        }

        pub fn format(key: &str, args: &$crate::Args) -> $crate::NativeResult<String> {
            LOCALIZER.format(key, args)
        }

        pub fn format_locale(
            locale: &str,
            key: &str,
            args: &$crate::Args,
        ) -> $crate::NativeResult<String> {
            LOCALIZER.format_locale(locale, key, args)
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

        pub fn negotiate_locale(preferred_locales: &[&str]) -> $crate::NativeResult<String> {
            LOCALIZER.negotiate_locale(preferred_locales.iter().copied())
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

        pub fn tr_locale(locale: &str, key: &str) -> $crate::NativeResult<String> {
            LOCALIZER.tr_locale(locale, key)
        }

        pub fn tr_with_args_locale(
            locale: &str,
            key: &str,
            args: &$crate::Args,
        ) -> $crate::NativeResult<String> {
            LOCALIZER.tr_with_args_locale(locale, key, args)
        }

        pub fn tr_or_key(key: &str) -> String {
            LOCALIZER.tr_or_key(key)
        }

        pub fn tr_with_args_or_key(key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args_or_key(key, args)
        }

        pub fn tr_locale_or_key(locale: &str, key: &str) -> String {
            LOCALIZER.tr_locale_or_key(locale, key)
        }

        pub fn tr_with_args_locale_or_key(locale: &str, key: &str, args: &$crate::Args) -> String {
            LOCALIZER.tr_with_args_locale_or_key(locale, key, args)
        }

        pub fn format(key: &str, args: &$crate::Args) -> $crate::NativeResult<String> {
            LOCALIZER.format(key, args)
        }

        pub fn format_locale(
            locale: &str,
            key: &str,
            args: &$crate::Args,
        ) -> $crate::NativeResult<String> {
            LOCALIZER.format_locale(locale, key, args)
        }
    };
}
