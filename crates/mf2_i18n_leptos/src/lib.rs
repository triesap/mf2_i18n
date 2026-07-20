#![forbid(unsafe_code)]

use leptos::prelude::{Effect, Get, LocalStorage, RwSignal, Set, provide_context, use_context};
pub use mf2_i18n_core::{Args, I18nMessageKey, RichTextMessage, RichTextPart};
pub use mf2_i18n_native::{NativeError, NativeLocalizer, NativeResult};

pub type I18nEffect = Effect<LocalStorage>;

pub trait I18nDocument: Clone + 'static {
    fn set_title(&self, title: &str);

    fn set_html_lang(&self, lang: &str);
}

#[derive(Clone, Copy, Default)]
pub struct BrowserI18nDocument;

#[cfg(target_arch = "wasm32")]
impl I18nDocument for BrowserI18nDocument {
    fn set_title(&self, title: &str) {
        leptos::prelude::document().set_title(title);
    }

    fn set_html_lang(&self, lang: &str) {
        if let Some(element) = leptos::prelude::document().document_element() {
            let _ = element.set_attribute("lang", lang);
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct I18nDocumentProjection {
    title: Option<String>,
    html_lang: Option<String>,
}

impl I18nDocumentProjection {
    pub fn new(title: Option<String>, html_lang: Option<String>) -> Self {
        Self { title, html_lang }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn html_lang(&self) -> Option<&str> {
        self.html_lang.as_deref()
    }

    pub fn apply_to<D>(&self, document: D)
    where
        D: I18nDocument,
    {
        if let Some(title) = self.title() {
            document.set_title(title);
        }
        if let Some(lang) = self.html_lang() {
            document.set_html_lang(lang);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichTextRenderNode<TSlot> {
    Text(String),
    Slot { name: String, value: TSlot },
}

#[derive(Clone)]
pub struct I18nContext {
    localizer: &'static NativeLocalizer,
    locale: RwSignal<String>,
}

impl I18nContext {
    pub fn new(localizer: &'static NativeLocalizer) -> Self {
        Self::new_with_locale(localizer, localizer.default_locale())
    }

    pub fn new_with_locale(localizer: &'static NativeLocalizer, locale: impl Into<String>) -> Self {
        Self {
            localizer,
            locale: RwSignal::new(locale.into()),
        }
    }

    pub fn try_new_with_locale(
        localizer: &'static NativeLocalizer,
        locale: &str,
    ) -> NativeResult<Self> {
        Self::try_new_with_preferred_locales(localizer, [locale])
    }

    pub fn try_new_with_preferred_locales<I, S>(
        localizer: &'static NativeLocalizer,
        preferred_locales: I,
    ) -> NativeResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let selected = localizer.negotiate_locale(preferred_locales)?;
        Ok(Self::new_with_locale(localizer, selected))
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

    pub fn try_rich_text_with_args<K>(&self, key: K, args: &Args) -> NativeResult<RichTextMessage>
    where
        K: I18nMessageKey,
    {
        let locale = self.locale();
        self.localizer
            .format_rich_text_locale(&locale, key.id(), args)
    }

    pub fn rich_text_with_args<K>(&self, key: K, args: &Args) -> RichTextMessage
    where
        K: I18nMessageKey,
    {
        self.try_rich_text_with_args(key, args)
            .expect("mf2_i18n leptos rich text formatting failed")
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

    pub fn try_document_projection<K>(
        &self,
        title_key: Option<K>,
        include_html_lang: bool,
    ) -> NativeResult<I18nDocumentProjection>
    where
        K: I18nMessageKey,
    {
        let title = title_key.map(|key| self.try_text(key)).transpose()?;
        let html_lang = include_html_lang.then(|| self.locale());
        Ok(I18nDocumentProjection::new(title, html_lang))
    }

    pub fn document_projection_or_key<K>(
        &self,
        title_key: Option<K>,
        include_html_lang: bool,
    ) -> I18nDocumentProjection
    where
        K: I18nMessageKey,
    {
        let title = title_key.map(|key| self.text_or_key(key));
        let html_lang = include_html_lang.then(|| self.locale());
        I18nDocumentProjection::new(title, html_lang)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update_document_title<K>(&self, key: K)
    where
        K: I18nMessageKey,
    {
        self.update_document_title_with(key, BrowserI18nDocument);
    }

    pub fn update_document_title_with<K, D>(&self, key: K, document: D)
    where
        K: I18nMessageKey,
        D: I18nDocument,
    {
        document.set_title(&self.text_or_key(key));
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sync_document_title<K>(&self, key: K) -> I18nEffect
    where
        K: I18nMessageKey,
    {
        self.sync_document_title_with(key, BrowserI18nDocument)
    }

    pub fn sync_document_title_with<K, D>(&self, key: K, document: D) -> I18nEffect
    where
        K: I18nMessageKey,
        D: I18nDocument,
    {
        let context = self.clone();
        Effect::new(move || {
            context.update_document_title_with(key, document.clone());
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update_html_lang(&self) {
        self.update_html_lang_with(BrowserI18nDocument);
    }

    pub fn update_html_lang_with<D>(&self, document: D)
    where
        D: I18nDocument,
    {
        document.set_html_lang(&self.locale());
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sync_html_lang(&self) -> I18nEffect {
        self.sync_html_lang_with(BrowserI18nDocument)
    }

    pub fn sync_html_lang_with<D>(&self, document: D) -> I18nEffect
    where
        D: I18nDocument,
    {
        let context = self.clone();
        Effect::new(move || {
            context.update_html_lang_with(document.clone());
        })
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
    let context = I18nContext::try_new_with_locale(localizer, locale)?;
    provide_context(context.clone());
    Ok(context)
}

pub fn provide_i18n_with_preferred_locales<I, S>(
    localizer: &'static NativeLocalizer,
    preferred_locales: I,
) -> NativeResult<I18nContext>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let context = I18nContext::try_new_with_preferred_locales(localizer, preferred_locales)?;
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

#[cfg(target_arch = "wasm32")]
pub fn sync_document_title<K>(key: K) -> I18nEffect
where
    K: I18nMessageKey,
{
    use_i18n().sync_document_title(key)
}

#[cfg(target_arch = "wasm32")]
pub fn sync_html_lang() -> I18nEffect {
    use_i18n().sync_html_lang()
}

pub fn render_rich_text<TSlot>(
    message: &RichTextMessage,
    mut slot: impl FnMut(&str) -> TSlot,
) -> Vec<RichTextRenderNode<TSlot>> {
    message
        .parts()
        .iter()
        .map(|part| match part {
            RichTextPart::Text(value) => RichTextRenderNode::Text(value.clone()),
            RichTextPart::Slot { name } => RichTextRenderNode::Slot {
                name: name.clone(),
                value: slot(name),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use leptos::prelude::{Get, Owner};

    use super::{
        I18nContext, I18nDocument, I18nDocumentProjection, I18nMessageKey, NativeLocalizer,
        RichTextMessage, RichTextPart, RichTextRenderNode, provide_i18n, provide_i18n_with_locale,
        provide_i18n_with_preferred_locales, render_rich_text, text_or_key, try_text, try_use_i18n,
        use_i18n,
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

    #[derive(Clone, Default)]
    struct RecordingDocument {
        title: Arc<RwLock<Vec<String>>>,
        lang: Arc<RwLock<Vec<String>>>,
    }

    impl I18nDocument for RecordingDocument {
        fn set_title(&self, title: &str) {
            self.title.write().expect("title").push(title.to_string());
        }

        fn set_html_lang(&self, lang: &str) {
            self.lang.write().expect("lang").push(lang.to_string());
        }
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
    fn default_context_uses_default_locale_not_native_active_locale() {
        Owner::new().with(|| {
            let localizer = test_localizer();
            localizer.set_locale("fr-CA").expect("native locale");
            let context = I18nContext::new(localizer);

            assert_eq!(localizer.locale(), "fr-CA");
            assert_eq!(localizer.default_locale(), "en");
            assert_eq!(context.locale(), "en");
        });
    }

    #[test]
    fn preferred_locale_constructors_negotiate_without_mutating_native_locale() {
        Owner::new().with(|| {
            let localizer = test_localizer();
            let context =
                I18nContext::try_new_with_preferred_locales(localizer, ["fr-CA", "en-GB"])
                    .expect("context");

            assert_eq!(context.locale(), "fr-CA");
            assert_eq!(localizer.locale(), "en");

            let provided =
                provide_i18n_with_preferred_locales(localizer, ["es-MX"]).expect("provided");

            assert_eq!(provided.locale(), "es-MX");
            assert_eq!(use_i18n().locale(), "es-MX");
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

    #[test]
    fn document_update_helpers_use_context_text_and_locale() {
        Owner::new().with(|| {
            let context = provide_i18n(test_localizer());
            let document = RecordingDocument::default();

            context.update_document_title_with(TestKey::HomeTitle, document.clone());
            context.set_locale("fr").expect("locale");
            context.update_html_lang_with(document.clone());

            assert_eq!(
                document.title.read().expect("title").as_slice(),
                ["home.title"]
            );
            assert_eq!(document.lang.read().expect("lang").as_slice(), ["fr"]);
        });
    }

    #[test]
    fn pure_document_projection_is_explicit_and_browser_independent() {
        Owner::new().with(|| {
            let context = provide_i18n(test_localizer());
            let projection = context.document_projection_or_key(Some(TestKey::HomeTitle), true);

            assert_eq!(
                projection,
                I18nDocumentProjection::new(Some("home.title".to_string()), Some("en".to_string()))
            );

            let document = RecordingDocument::default();
            projection.apply_to(document.clone());
            assert_eq!(
                document.title.read().expect("title").as_slice(),
                ["home.title"]
            );
            assert_eq!(document.lang.read().expect("lang").as_slice(), ["en"]);
        });
    }

    #[test]
    fn document_sync_helpers_compile_against_owner_context() {
        Owner::new().with(|| {
            let context = provide_i18n(test_localizer());
            let document = RecordingDocument::default();

            let _title = context.sync_document_title_with(TestKey::HomeTitle, document.clone());
            let _lang = context.sync_html_lang_with(document);
        });
    }

    #[test]
    fn rich_text_rendering_preserves_named_slot_order() {
        let message = RichTextMessage::new(vec![
            RichTextPart::text("Use "),
            RichTextPart::slot("ios"),
            RichTextPart::text(" or "),
            RichTextPart::slot("android"),
            RichTextPart::text("."),
        ]);

        let rendered = render_rich_text(&message, |name| format!("<{name}>"));

        assert_eq!(
            rendered,
            vec![
                RichTextRenderNode::Text("Use ".to_string()),
                RichTextRenderNode::Slot {
                    name: "ios".to_string(),
                    value: "<ios>".to_string(),
                },
                RichTextRenderNode::Text(" or ".to_string()),
                RichTextRenderNode::Slot {
                    name: "android".to_string(),
                    value: "<android>".to_string(),
                },
                RichTextRenderNode::Text(".".to_string()),
            ]
        );
    }
}
