use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn facade_crate_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn fixture_manifest(name: &str) -> PathBuf {
    facade_crate_path()
        .join("tests")
        .join("fixtures")
        .join(name)
        .join("Cargo.toml")
}

fn unique_target_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    path.push(format!("mf2_i18n_consumer_mode_{name}_{nanos}"));
    path
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn assert_manifest_builds(name: &str, manifest_path: &Path) {
    assert_manifest_command_succeeds(name, manifest_path, "check");
}

fn assert_manifest_tests(name: &str, manifest_path: &Path) {
    assert_manifest_command_succeeds(name, manifest_path, "test");
}

fn assert_manifest_command_succeeds(name: &str, manifest_path: &Path, command: &str) {
    let output = Command::new(cargo_bin())
        .arg(command)
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target-dir")
        .arg(unique_target_dir(name))
        .output()
        .expect("fixture cargo check should run");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("fixture `{name}` failed cargo {command}\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }
}

fn assert_fixture_builds(name: &str) {
    assert_manifest_builds(name, &fixture_manifest(name));
}

fn assert_inline_fixture_builds(name: &str, features: &str, source: &str) {
    let fixture_dir = unique_target_dir(&format!("{name}_fixture"));
    let src_dir = fixture_dir.join("src");
    fs::create_dir_all(&src_dir).expect("src");
    fs::write(
        fixture_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "mf2_i18n_facade_{name}_fixture"
version = "0.0.0"
edition = "2024"

[workspace]

[dependencies]
mf2_i18n = {{ path = {path:?}, default-features = false, features = {features} }}
"#,
            path = facade_crate_path().to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("manifest");
    fs::write(src_dir.join("lib.rs"), source).expect("source");
    assert_manifest_builds(name, &fixture_dir.join("Cargo.toml"));
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, contents).expect("write");
}

#[test]
fn supports_core_consumer_mode() {
    assert_inline_fixture_builds(
        "core_only",
        "[]",
        r#"
pub fn smoke() -> mf2_i18n::Args {
    mf2_i18n::Args::new()
}
"#,
    );
}

#[test]
fn supports_runtime_only_consumer_mode() {
    assert_inline_fixture_builds(
        "runtime_only",
        r#"["runtime"]"#,
        r#"
pub fn smoke() {
    let _runtime: Option<mf2_i18n::Runtime> = None;
    let _manifest: Option<mf2_i18n::Manifest> = None;
}
"#,
    );
}

#[test]
fn supports_native_consumer_mode() {
    assert_fixture_builds("runtime_only");
}

#[test]
fn supports_build_only_consumer_mode() {
    assert_fixture_builds("build_only");
}

#[test]
fn supports_mixed_runtime_and_build_consumer_mode() {
    assert_fixture_builds("mixed_runtime_build");
}

#[test]
fn supports_leptos_consumer_mode() {
    assert_inline_fixture_builds(
        "leptos",
        r#"["leptos"]"#,
        r#"
#[derive(Clone, Copy)]
enum Key {
    HomeTitle,
}

impl mf2_i18n::leptos::I18nMessageKey for Key {
    fn id(self) -> &'static str {
        match self {
            Self::HomeTitle => "home.title",
        }
    }
}

pub fn smoke(context: mf2_i18n::leptos::I18nContext) -> String {
    context.text_or_key(Key::HomeTitle)
}
"#,
    );
}

#[test]
fn supports_homepage_shaped_leptos_consumer_workflow() {
    let fixture_dir = unique_target_dir("homepage_leptos_fixture");
    write_file(
        &fixture_dir.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "mf2_i18n_homepage_leptos_fixture"
version = "0.0.0"
edition = "2024"

[workspace]

[dependencies]
mf2_i18n = {{ path = {path:?}, default-features = false, features = ["leptos"] }}
leptos = {{ version = "0.9.0-alpha", default-features = false }}

[build-dependencies]
mf2_i18n = {{ path = {path:?}, default-features = false, features = ["build"] }}
"#,
            path = facade_crate_path().to_string_lossy().replace('\\', "/")
        ),
    );
    write_file(&fixture_dir.join("id_salt.txt"), "homepage-leptos-fixture");
    write_file(
        &fixture_dir.join("mf2_i18n.toml"),
        r#"version = 1
default_locale = "en"
locales = ["en", "es"]
project_salt_path = "id_salt.txt"

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
namespace_glob = "*.mf2"

[[native.targets]]
name = "app-native"
out_dir = "target/mf2/native"
module_name = "app_i18n"

[[leptos.targets]]
name = "app"
native_target = "app-native"
out_dir = "src/generated/i18n"
module_name = "app_i18n"
document_title_key = "homepage.title"
sync_html_lang = true
"#,
    );
    write_file(
        &fixture_dir.join("locales/en/homepage.mf2"),
        "title = Radroots\n\nwelcome = Welcome { $name }\n\ncta = Use { $ios :slot } or { $android :slot }.\n",
    );
    write_file(
        &fixture_dir.join("locales/es/homepage.mf2"),
        "title = Raices\n\nwelcome = Hola { $name }\n\ncta = Usa { $android :slot } o { $ios :slot }.\n",
    );
    write_file(
        &fixture_dir.join("build.rs"),
        r#"fn main() {
    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let config_path = manifest_dir.join("mf2_i18n.toml");
    let output = mf2_i18n::build_leptos_target(
        &mf2_i18n::LeptosTargetBuildOptions::new(&config_path, "app").with_out_dir(&out_dir),
    )
    .expect("leptos target");

    for path in output.native_output().rerun_if_changed_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
"#,
    );
    write_file(
        &fixture_dir.join("src/lib.rs"),
        r#"include!(concat!(env!("OUT_DIR"), "/app_i18n.rs"));

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use leptos::prelude::Owner;
    use mf2_i18n::leptos::{I18nDocument, RichTextMessage, RichTextPart, RichTextRenderNode};

    use super::{
        HomepageCtaRichArgs, HomepageWelcomeArgs, LEPTOS_MODULE_NAME, MessageKey,
        leptos_document_title_key, leptos_sync_html_lang, localizer, rich_homepage_cta,
        t_homepage_title, t_homepage_welcome, text_homepage_title, text_homepage_title_or_key,
        text_homepage_title_signal, text_homepage_welcome, text_homepage_welcome_signal,
        try_text_homepage_title, try_text_homepage_welcome, sync_leptos_document_with,
    };

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
    fn generated_native_helpers_format_homepage_text() {
        let args = HomepageWelcomeArgs::new("Nova");

        assert_eq!(LEPTOS_MODULE_NAME, "app_i18n");
        assert_eq!(leptos_document_title_key(), Some(MessageKey::HomepageTitle));
        assert!(leptos_sync_html_lang());
        assert_eq!(t_homepage_title(), "Radroots");
        assert_eq!(t_homepage_welcome(&args), "Welcome Nova");
    }

    #[test]
    fn leptos_context_formats_generated_keys_with_context_locale() {
        Owner::new().with(|| {
            let context = mf2_i18n::leptos::provide_i18n_with_locale(localizer(), "es")
                .expect("context");
            let args = HomepageWelcomeArgs::new("Nova");
            let title_signal = text_homepage_title_signal(&context);
            let welcome_signal =
                text_homepage_welcome_signal(&context, || HomepageWelcomeArgs::new("Nova"));

            assert_eq!(context.text_or_key(MessageKey::HomepageTitle), "Raices");
            assert_eq!(
                context.text_with_args(MessageKey::HomepageWelcome, args.as_args()),
                "Hola Nova"
            );
            assert_eq!(try_text_homepage_title(&context).expect("title"), "Raices");
            assert_eq!(text_homepage_title(&context), "Raices");
            assert_eq!(text_homepage_title_or_key(&context), "Raices");
            assert_eq!(
                try_text_homepage_welcome(&context, &args).expect("welcome"),
                "Hola Nova"
            );
            assert_eq!(text_homepage_welcome(&context, &args), "Hola Nova");
            assert_eq!(title_signal(), "Raices");
            assert_eq!(welcome_signal(), "Hola Nova");

            context.set_locale("en").expect("locale");

            assert_eq!(title_signal(), "Radroots");
            assert_eq!(welcome_signal(), "Welcome Nova");
        });
    }

    #[test]
    fn document_helpers_sync_title_and_lang_from_context() {
        Owner::new().with(|| {
            let context = mf2_i18n::leptos::provide_i18n_with_locale(localizer(), "es")
                .expect("context");
            let document = RecordingDocument::default();

            context.update_document_title_with(MessageKey::HomepageTitle, document.clone());
            context.update_html_lang_with(document.clone());
            let effects = sync_leptos_document_with(&context, document.clone());

            assert_eq!(document.title.read().expect("title").as_slice(), ["Raices"]);
            assert_eq!(document.lang.read().expect("lang").as_slice(), ["es"]);
            assert!(effects.title.is_some());
            assert!(effects.html_lang.is_some());
        });
    }

    #[test]
    fn rich_text_rendering_contract_preserves_slot_order() {
        let message = RichTextMessage::new(vec![
            RichTextPart::text("Use "),
            RichTextPart::slot("ios"),
            RichTextPart::text(" or "),
            RichTextPart::slot("android"),
            RichTextPart::text("."),
        ]);
        let rendered = mf2_i18n::leptos::render_rich_text(&message, |name| name.to_string());

        assert_eq!(
            rendered,
            vec![
                RichTextRenderNode::Text("Use ".to_string()),
                RichTextRenderNode::Slot {
                    name: "ios".to_string(),
                    value: "ios".to_string(),
                },
                RichTextRenderNode::Text(" or ".to_string()),
                RichTextRenderNode::Slot {
                    name: "android".to_string(),
                    value: "android".to_string(),
                },
                RichTextRenderNode::Text(".".to_string()),
            ]
        );
    }

    #[test]
    fn generated_rich_text_helpers_preserve_localized_slot_order() {
        Owner::new().with(|| {
            let context = mf2_i18n::leptos::provide_i18n_with_locale(localizer(), "es")
                .expect("context");
            let args = HomepageCtaRichArgs::new("ios".to_string(), "android".to_string());
            let rendered = rich_homepage_cta(&context, &args);

            assert_eq!(
                rendered,
                vec![
                    RichTextRenderNode::Text("Usa ".to_string()),
                    RichTextRenderNode::Slot {
                        name: "android".to_string(),
                        value: "android".to_string(),
                    },
                    RichTextRenderNode::Text(" o ".to_string()),
                    RichTextRenderNode::Slot {
                        name: "ios".to_string(),
                        value: "ios".to_string(),
                    },
                    RichTextRenderNode::Text(".".to_string()),
                ]
            );
        });
    }
}
"#,
    );

    assert_manifest_tests("homepage_leptos", &fixture_dir.join("Cargo.toml"));
}
