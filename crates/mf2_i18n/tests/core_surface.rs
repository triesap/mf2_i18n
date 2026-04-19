use mf2_i18n::{Args, LanguageTag, Value, negotiate_lookup};

#[test]
fn root_exports_core_surface() {
    let mut args = Args::new();
    args.insert("name", Value::Str("Nova".to_string()));

    let value = args.get("name").expect("value should exist");
    match value {
        Value::Str(value) => assert_eq!(value, "Nova"),
        other => panic!("unexpected value type: {other:?}"),
    }

    let requested = [LanguageTag::parse("fr-CA").expect("requested locale")];
    let supported = [
        LanguageTag::parse("en").expect("english locale"),
        LanguageTag::parse("fr").expect("french locale"),
    ];
    let default_locale = LanguageTag::parse("en").expect("default locale");

    let negotiation = negotiate_lookup(&requested, &supported, &default_locale);
    assert_eq!(negotiation.selected.normalized(), "fr");
}
