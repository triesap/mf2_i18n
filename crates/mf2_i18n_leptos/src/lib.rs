#![forbid(unsafe_code)]

use leptos::prelude::{Get, RwSignal, Set, provide_context, use_context};
pub use mf2_i18n_core::{Args, I18nMessageKey, RichTextMessage, RichTextPart};
pub use mf2_i18n_native::{NativeError, NativeLocalizer, NativeResult};

#[derive(Clone)]
pub struct I18nContext {
    localizer: &'static NativeLocalizer,
    locale: RwSignal<String>,
}

impl I18nContext {
    pub fn new(localizer: &'static NativeLocalizer) -> Self {
        Self::new_with_locale(localizer, localizer.locale())
    }

    pub fn new_with_locale(localizer: &'static NativeLocalizer, locale: impl Into<String>) -> Self {
        Self {
            localizer,
            locale: RwSignal::new(locale.into()),
        }
    }

    pub fn localizer(&self) -> &'static NativeLocalizer {
        self.localizer
    }

    pub fn locale_signal(&self) -> RwSignal<String> {
        self.locale
    }

    pub fn locale(&self) -> String {
        self.locale.get()
    }

    pub fn set_locale(&self, locale: &str) -> NativeResult<String> {
        let selected = self.localizer.negotiate_locale([locale])?;
        self.locale.set(selected.clone());
        Ok(selected)
    }

    pub fn set_preferred_locales<I, S>(&self, preferred_locales: I) -> NativeResult<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let selected = self.localizer.negotiate_locale(preferred_locales)?;
        self.locale.set(selected.clone());
        Ok(selected)
    }

    pub fn try_text<K>(&self, key: K) -> NativeResult<String>
    where
        K: I18nMessageKey,
    {
        let locale = self.locale();
        self.localizer.tr_locale(&locale, key.id())
    }

    pub fn text<K>(&self, key: K) -> String
    where
        K: I18nMessageKey,
    {
        self.try_text(key)
            .expect("mf2_i18n leptos message formatting failed")
    }

    pub fn text_or_key<K>(&self, key: K) -> String
    where
        K: I18nMessageKey,
    {
        let locale = self.locale();
        self.localizer.tr_locale_or_key(&locale, key.id())
    }

    pub fn try_text_with_args<K>(&self, key: K, args: &Args) -> NativeResult<String>
    where
        K: I18nMessageKey,
    {
        let locale = self.locale();
        self.localizer.tr_with_args_locale(&locale, key.id(), args)
    }

    pub fn text_with_args<K>(&self, key: K, args: &Args) -> String
    where
        K: I18nMessageKey,
    {
        self.try_text_with_args(key, args)
            .expect("mf2_i18n leptos message formatting failed")
    }

    pub fn text_with_args_or_key<K>(&self, key: K, args: &Args) -> String
    where
        K: I18nMessageKey,
    {
        let locale = self.locale();
        self.localizer
            .tr_with_args_locale_or_key(&locale, key.id(), args)
    }

    pub fn try_text_signal<K>(
        &self,
        key: K,
    ) -> impl Fn() -> NativeResult<String> + Clone + 'static + use<K>
    where
        K: I18nMessageKey,
    {
        let context = self.clone();
        move || context.try_text(key)
    }

    pub fn text_signal<K>(&self, key: K) -> impl Fn() -> String + Clone + 'static + use<K>
    where
        K: I18nMessageKey,
    {
        let context = self.clone();
        move || context.text(key)
    }

    pub fn text_or_key_signal<K>(&self, key: K) -> impl Fn() -> String + Clone + 'static + use<K>
    where
        K: I18nMessageKey,
    {
        let context = self.clone();
        move || context.text_or_key(key)
    }
}

pub fn provide_i18n(localizer: &'static NativeLocalizer) -> I18nContext {
    let context = I18nContext::new(localizer);
    provide_context(context.clone());
    context
}

pub fn provide_i18n_with_locale(
    localizer: &'static NativeLocalizer,
    locale: &str,
) -> NativeResult<I18nContext> {
    let selected = localizer.negotiate_locale([locale])?;
    let context = I18nContext::new_with_locale(localizer, selected);
    provide_context(context.clone());
    Ok(context)
}

pub fn try_use_i18n() -> Option<I18nContext> {
    use_context::<I18nContext>()
}

pub fn use_i18n() -> I18nContext {
    try_use_i18n().expect("mf2_i18n leptos context was not provided")
}

pub fn try_text<K>(key: K) -> NativeResult<String>
where
    K: I18nMessageKey,
{
    use_i18n().try_text(key)
}

pub fn text<K>(key: K) -> String
where
    K: I18nMessageKey,
{
    use_i18n().text(key)
}

pub fn text_or_key<K>(key: K) -> String
where
    K: I18nMessageKey,
{
    use_i18n().text_or_key(key)
}

pub fn try_text_signal<K>(key: K) -> impl Fn() -> NativeResult<String> + Clone + 'static + use<K>
where
    K: I18nMessageKey,
{
    use_i18n().try_text_signal(key)
}

pub fn text_signal<K>(key: K) -> impl Fn() -> String + Clone + 'static + use<K>
where
    K: I18nMessageKey,
{
    use_i18n().text_signal(key)
}

pub fn text_or_key_signal<K>(key: K) -> impl Fn() -> String + Clone + 'static + use<K>
where
    K: I18nMessageKey,
{
    use_i18n().text_or_key_signal(key)
}

#[cfg(test)]
mod tests {
    use leptos::prelude::{Get, Owner};

    use super::{
        I18nMessageKey, NativeLocalizer, provide_i18n, provide_i18n_with_locale, text_or_key,
        try_text, try_use_i18n, use_i18n,
    };

    #[derive(Clone, Copy)]
    enum TestKey {
        HomeTitle,
    }

    impl I18nMessageKey for TestKey {
        fn id(self) -> &'static str {
            match self {
                Self::HomeTitle => "home.title",
            }
        }
    }

    fn test_localizer() -> &'static NativeLocalizer {
        Box::leak(Box::new(NativeLocalizer::fallback("en")))
    }

    #[test]
    fn provides_and_uses_i18n_context() {
        Owner::new().with(|| {
            let localizer = test_localizer();
            let context = provide_i18n(localizer);

            assert!(try_use_i18n().is_some());
            assert_eq!(use_i18n().locale(), "en");
            assert_eq!(context.text_or_key(TestKey::HomeTitle), "home.title");
            assert_eq!(text_or_key(TestKey::HomeTitle), "home.title");
            assert!(try_text(TestKey::HomeTitle).is_err());
        });
    }

    #[test]
    fn context_locale_state_does_not_mutate_native_global_locale() {
        Owner::new().with(|| {
            let localizer = test_localizer();
            let context = provide_i18n_with_locale(localizer, "fr-CA").expect("context");

            assert_eq!(context.locale(), "fr-CA");
            assert_eq!(localizer.locale(), "en");

            let selected = context.set_locale("en-US").expect("selected");
            assert_eq!(selected, "en-US");
            assert_eq!(context.locale(), "en-US");
            assert_eq!(localizer.locale(), "en");
        });
    }

    #[test]
    fn text_signal_tracks_context_locale() {
        Owner::new().with(|| {
            let context = provide_i18n(test_localizer());
            let text = context.text_or_key_signal(TestKey::HomeTitle);

            assert_eq!(text(), "home.title");
            context.set_locale("fr").expect("locale");
            assert_eq!(text(), "home.title");
            assert_eq!(context.locale_signal().get(), "fr");
        });
    }
}
