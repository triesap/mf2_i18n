use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mf2_i18n_build::{
    DocusaurusWebBuildOptions, HybridWebBuildOptions, PlainJsonWebBuildOptions,
    RuntimeWebBuildOptions, build_docusaurus_web_target, build_hybrid_web_target,
    build_plain_json_web_target, build_runtime_web_target,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "mf2_i18n_web_fixture_{}_{name}_{nanos}_{counter}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("dir");
    path
}

fn write_json_catalog(path: &Path, messages: &[(&str, &str)]) {
    let contents = messages
        .iter()
        .map(|(key, value)| format!("  {key:?}: {value:?}"))
        .collect::<Vec<_>>()
        .join(",\n");
    fs::write(path, format!("{{\n{contents}\n}}\n")).expect("write");
}

#[test]
fn fixture_projects_cover_plain_runtime_docusaurus_and_hybrid_targets() {
    let plain = write_plain_fixture();
    let plain_output =
        build_plain_json_web_target(&PlainJsonWebBuildOptions::new(&plain, "plain-html"))
            .expect("plain");
    let plain_root = plain.parent().expect("plain root");
    assert_eq!(
        plain_output.artifact_output().manifest().target,
        "plain-html"
    );
    assert!(plain_root.join("web/plain/messages/en/home.json").exists());
    assert!(plain_root.join("web/plain/types.ts").exists());

    let runtime = write_runtime_fixture();
    let runtime_output =
        build_runtime_web_target(&RuntimeWebBuildOptions::new(&runtime, "vite-runtime"))
            .expect("runtime");
    let runtime_root = runtime.parent().expect("runtime root");
    assert_eq!(
        runtime_output.runtime_assets().supported_locales,
        vec!["en".to_string(), "es".to_string()]
    );
    assert!(
        runtime_root
            .join("web/runtime/runtime-assets.json")
            .exists()
    );
    assert!(runtime_root.join("web/runtime/packs/en.mf2pack").exists());
    let runtime_loaders =
        fs::read_to_string(runtime_root.join("web/runtime/loaders.ts")).expect("loaders");
    assert!(runtime_loaders.contains("createMf2Runtime"));
    assert!(runtime_loaders.contains("loadLocale"));

    let hybrid = write_hybrid_fixture();
    let hybrid_output =
        build_hybrid_web_target(&HybridWebBuildOptions::new(&hybrid, "vite-hybrid"))
            .expect("hybrid");
    let hybrid_root = hybrid.parent().expect("hybrid root");
    assert_eq!(
        hybrid_output.hybrid_assets().static_message_ids,
        vec!["home.title".to_string()]
    );
    assert_eq!(
        hybrid_output.hybrid_assets().runtime_message_ids,
        vec!["home.count".to_string()]
    );
    assert!(hybrid_root.join("web/hybrid/hybrid-assets.json").exists());
    assert!(hybrid_root.join("web/hybrid/runtime-assets.json").exists());
    assert!(
        hybrid_root
            .join("web/hybrid/messages/en/home.json")
            .exists()
    );
    assert!(hybrid_root.join("web/hybrid/packs/en.mf2pack").exists());
    let hybrid_loaders =
        fs::read_to_string(hybrid_root.join("web/hybrid/loaders.ts")).expect("hybrid loaders");
    assert!(hybrid_loaders.contains("STATIC_MESSAGE_LOADERS"));
    assert!(hybrid_loaders.contains("createMf2Runtime"));
    assert!(hybrid_loaders.contains("export async function format"));

    let docusaurus = write_docusaurus_fixture();
    let docusaurus_output =
        build_docusaurus_web_target(&DocusaurusWebBuildOptions::new(&docusaurus, "docs"))
            .expect("docusaurus");
    let docusaurus_root = docusaurus.parent().expect("docusaurus root");
    assert_eq!(
        docusaurus_output.supported_locales(),
        &["en".to_string(), "es".to_string()]
    );
    assert!(docusaurus_root.join("generated/messages.ts").exists());
    assert!(docusaurus_root.join("i18n/es/code.json").exists());
    assert!(
        docusaurus_root
            .join("i18n/es/docusaurus-plugin-content-docs/current/guide/intro.mdx")
            .exists()
    );

    fs::remove_dir_all(plain_root).ok();
    fs::remove_dir_all(runtime_root).ok();
    fs::remove_dir_all(hybrid_root).ok();
    fs::remove_dir_all(docusaurus_root).ok();
}

fn write_plain_fixture() -> PathBuf {
    let root = temp_dir("plain");
    for locale in ["en", "es"] {
        let locale_dir = root.join("locales").join(locale);
        fs::create_dir_all(&locale_dir).expect("locale");
        write_json_catalog(&locale_dir.join("home.json"), &[("title", "value")]);
    }
    fs::write(root.join("id_salt.txt"), "salt").expect("salt");
    let config_path = root.join("mf2_i18n.toml");
    fs::write(
        &config_path,
        "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"plain-html\"\nmode = \"plain-json\"\nframework = \"vanilla\"\nout_dir = \"web/plain\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"home\"]\n",
    )
    .expect("config");
    config_path
}

fn write_runtime_fixture() -> PathBuf {
    let root = temp_dir("runtime");
    for locale in ["en", "es"] {
        let locale_dir = root.join("locales").join(locale);
        fs::create_dir_all(&locale_dir).expect("locale");
        write_json_catalog(
            &locale_dir.join("home.json"),
            &[("title", "Hi { $name }"), ("count", "{ $count :number }")],
        );
    }
    fs::write(root.join("id_salt.txt"), "salt").expect("salt");
    let config_path = root.join("mf2_i18n.toml");
    fs::write(
        &config_path,
        "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"vite-runtime\"\nmode = \"runtime\"\nframework = \"vite\"\nout_dir = \"web/runtime\"\nrelease_id = \"fixture-runtime\"\ngenerated_at = \"2026-02-01T00:00:00Z\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"home\"]\n",
    )
    .expect("config");
    config_path
}

fn write_hybrid_fixture() -> PathBuf {
    let root = temp_dir("hybrid");
    for locale in ["en", "es"] {
        let locale_dir = root.join("locales").join(locale);
        fs::create_dir_all(&locale_dir).expect("locale");
        write_json_catalog(
            &locale_dir.join("home.json"),
            &[("title", "Welcome"), ("count", "{ $count :number }")],
        );
    }
    fs::write(root.join("id_salt.txt"), "salt").expect("salt");
    let config_path = root.join("mf2_i18n.toml");
    fs::write(
        &config_path,
        "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"vite-hybrid\"\nmode = \"hybrid\"\nframework = \"vite\"\nout_dir = \"web/hybrid\"\nrelease_id = \"fixture-hybrid\"\ngenerated_at = \"2026-02-01T00:00:00Z\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"home\"]\n",
    )
    .expect("config");
    config_path
}

fn write_docusaurus_fixture() -> PathBuf {
    let root = temp_dir("docusaurus");
    for locale in ["en", "es"] {
        fs::create_dir_all(root.join("locales").join(locale)).expect("locale");
    }
    fs::create_dir_all(root.join("docs").join("guide")).expect("docs");
    fs::write(root.join("docs/guide/intro.mdx"), "# Intro\n").expect("source doc");
    fs::create_dir_all(root.join("locales/es/docs/guide")).expect("docs es");
    fs::write(root.join("id_salt.txt"), "salt").expect("salt");
    fs::write(root.join("locales/en/home.mf2"), "title = Docs\n").expect("home en");
    fs::write(root.join("locales/es/home.mf2"), "title = Documentos\n").expect("home es");
    fs::write(
        root.join("locales/es/docs/guide/intro.mf2"),
        "content = ---\ntitle: Intro\n---\n# Intro ES\n",
    )
    .expect("docs es");
    fs::write(
        root.join("locales/en/docusaurus.mf2"),
        "navbar.overview = Overview\n\ndocs.sidebar.overview = Overview\n",
    )
    .expect("docusaurus en");
    fs::write(
        root.join("locales/es/docusaurus.mf2"),
        "navbar.overview = Introduccion\n\ndocs.sidebar.overview = Introduccion\n",
    )
    .expect("docusaurus es");
    let config_path = root.join("mf2_i18n.toml");
    fs::write(
        &config_path,
        "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"docs\"\nmode = \"docusaurus\"\nframework = \"docusaurus\"\nout_dir = \"generated\"\ndocs_source_dir = \"docs\"\ndocusaurus_i18n_dir = \"i18n\"\ndocusaurus_sidebar_id = \"guideSidebar\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"home\", \"docs\", \"docusaurus\"]\n",
    )
    .expect("config");
    config_path
}
