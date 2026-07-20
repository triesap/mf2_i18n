use leptos::prelude::*;
use mf2_i18n::leptos::I18nDocumentProjection;

#[component]
pub fn LocalizedDocumentProbe(projection: I18nDocumentProjection) -> impl IntoView {
    let title = projection.title().unwrap_or_default().to_owned();
    let lang = projection.html_lang().unwrap_or_default().to_owned();

    view! {
        <main lang=lang>
            <h1>{title}</h1>
        </main>
    }
}

#[cfg(feature = "ssr")]
pub fn render_localized_document(projection: I18nDocumentProjection) -> String {
    view! { <LocalizedDocumentProbe projection/> }.to_html()
}

#[cfg(test)]
mod tests {
    use mf2_i18n::leptos::I18nDocumentProjection;

    #[cfg(feature = "ssr")]
    #[test]
    fn native_ssr_renders_the_pure_document_projection() {
        let html = super::render_localized_document(I18nDocumentProjection::new(
            Some("Radroots".to_owned()),
            Some("en".to_owned()),
        ));

        assert!(html.contains("<main lang=\"en\">"));
        assert!(html.contains("<h1>Radroots</h1>"));
    }
}
