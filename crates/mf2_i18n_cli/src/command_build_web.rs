use std::path::PathBuf;

use mf2_i18n_build::{
    DocusaurusWebBuildError, DocusaurusWebBuildOptions, HybridWebBuildError, HybridWebBuildOptions,
    PlainJsonWebBuildError, PlainJsonWebBuildOptions, ProjectError, ProjectLayout,
    RuntimeWebBuildError, RuntimeWebBuildOptions, WebTargetConfigError, WebTargetMode,
    build_docusaurus_web_target, build_hybrid_web_target, build_plain_json_web_target,
    build_runtime_web_target,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildWebCommandError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] WebTargetConfigError),
    #[error(transparent)]
    PlainJson(#[from] PlainJsonWebBuildError),
    #[error(transparent)]
    Runtime(#[from] RuntimeWebBuildError),
    #[error(transparent)]
    Docusaurus(#[from] DocusaurusWebBuildError),
    #[error(transparent)]
    Hybrid(#[from] HybridWebBuildError),
    #[error("missing web target {0}")]
    MissingTarget(String),
}

#[derive(Debug, Clone)]
pub struct BuildWebOptions {
    pub config_path: PathBuf,
    pub target_name: String,
    pub out_dir: Option<PathBuf>,
    pub check: bool,
    pub dto_types: bool,
}

pub fn run_build_web(options: &BuildWebOptions) -> Result<(), BuildWebCommandError> {
    let project = ProjectLayout::load_or_default(&options.config_path)?;
    project.config().validate_web_targets()?;
    let target = project
        .config()
        .web
        .target(&options.target_name)
        .ok_or_else(|| BuildWebCommandError::MissingTarget(options.target_name.clone()))?;

    match target.mode {
        WebTargetMode::PlainJson => {
            let mut build_options =
                PlainJsonWebBuildOptions::new(&options.config_path, &options.target_name)
                    .with_check(options.check)
                    .with_dto_types(options.dto_types);

            if let Some(out_dir) = &options.out_dir {
                build_options = build_options.with_out_dir(out_dir);
            }

            build_plain_json_web_target(&build_options)?;
        }
        WebTargetMode::Runtime => {
            let mut build_options =
                RuntimeWebBuildOptions::new(&options.config_path, &options.target_name)
                    .with_check(options.check)
                    .with_dto_types(options.dto_types);

            if let Some(out_dir) = &options.out_dir {
                build_options = build_options.with_out_dir(out_dir);
            }

            build_runtime_web_target(&build_options)?;
        }
        WebTargetMode::Docusaurus => {
            let mut build_options =
                DocusaurusWebBuildOptions::new(&options.config_path, &options.target_name)
                    .with_check(options.check)
                    .with_dto_types(options.dto_types);

            if let Some(out_dir) = &options.out_dir {
                build_options = build_options.with_out_dir(out_dir);
            }

            build_docusaurus_web_target(&build_options)?;
        }
        WebTargetMode::Hybrid => {
            let mut build_options =
                HybridWebBuildOptions::new(&options.config_path, &options.target_name)
                    .with_check(options.check)
                    .with_dto_types(options.dto_types);

            if let Some(out_dir) = &options.out_dir {
                build_options = build_options.with_out_dir(out_dir);
            }

            build_hybrid_web_target(&build_options)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BuildWebOptions, run_build_web};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!(
            "mf2_i18n_cli_build_web_{}_{nanos}_{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn builds_named_web_target() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "plain-json"
out_dir = "web-generated"
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path: config_path.clone(),
            target_name: "docs".to_string(),
            out_dir: None,
            check: false,
            dto_types: false,
        })
        .expect("build");

        let out_dir = dir.join("web-generated");
        assert!(out_dir.join("mf2-web-manifest.json").exists());
        assert!(out_dir.join("types.ts").exists());
        assert!(out_dir.join("messages/en/common.json").exists());

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "docs".to_string(),
            out_dir: Some(out_dir),
            check: true,
            dto_types: false,
        })
        .expect("check");

        fs::remove_dir_all(&dir).ok();
    }

    #[cfg(feature = "dto-bindgen")]
    #[test]
    fn builds_named_web_target_with_dto_types() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "plain-json"
out_dir = "web-generated"
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path: config_path.clone(),
            target_name: "docs".to_string(),
            out_dir: None,
            check: false,
            dto_types: true,
        })
        .expect("build");

        let out_dir = dir.join("web-generated");
        assert!(out_dir.join("dto/dto_bindgen.generated.json").exists());
        assert!(out_dir.join("dto/ts/catalog.ts").exists());
        assert!(out_dir.join("dto/ts/index.ts").exists());
        assert!(!out_dir.join("dto/python").exists());
        let index = fs::read_to_string(out_dir.join("index.ts")).expect("index");
        assert!(index.contains("export * from \"./types\";"));
        assert!(index.contains("export * from \"./loaders\";"));
        assert!(index.contains("Manifest as Mf2WebManifest"));
        assert!(index.contains("Catalog as Mf2Catalog"));
        assert!(index.contains("RuntimeWebAssets as Mf2RuntimeAssets"));
        let manifest = fs::read_to_string(out_dir.join("mf2-web-manifest.json")).expect("manifest");
        assert!(manifest.contains("index.ts"));
        assert!(manifest.contains("dto/ts/catalog.ts"));
        assert!(!manifest.contains("dto/python"));

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "docs".to_string(),
            out_dir: Some(out_dir),
            check: true,
            dto_types: true,
        })
        .expect("check");

        fs::remove_dir_all(&dir).ok();
    }

    #[cfg(feature = "dto-bindgen")]
    #[test]
    fn generated_dto_typescript_typechecks_when_requested() {
        let tsc = match std::env::var("MF2_I18N_TSC") {
            Ok(path) => path,
            Err(_) if std::env::var("MF2_I18N_RUN_TSC").is_ok() => "tsc".to_string(),
            Err(_) => return,
        };
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "plain-json"
out_dir = "web-generated"
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "docs".to_string(),
            out_dir: None,
            check: false,
            dto_types: true,
        })
        .expect("build");

        fs::write(
            dir.join("tsconfig.dto.json"),
            r#"{
  "compilerOptions": {
    "strict": true,
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["web-generated/dto/ts/**/*.ts"]
}
"#,
        )
        .expect("tsconfig");
        let tsc_path = PathBuf::from(&tsc);
        let tsc_command = if tsc_path.components().count() > 1 {
            fs::canonicalize(&tsc_path).unwrap_or(tsc_path)
        } else {
            tsc_path
        };
        let output = Command::new(&tsc_command)
            .arg("-p")
            .arg(dir.join("tsconfig.dto.json"))
            .current_dir(&dir)
            .output()
            .unwrap_or_else(|source| panic!("failed to run {tsc}: {source}"));
        assert!(
            output.status.success(),
            "{tsc} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builds_runtime_web_target() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(
            locales_dir.join("common.json"),
            r#"{"home":{"title":"Hi"}}"#,
        )
        .expect("write");
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "runtime"
mode = "runtime"
out_dir = "web-runtime"
release_id = "test-runtime"
generated_at = "2026-02-01T00:00:00Z"
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "runtime".to_string(),
            out_dir: None,
            check: false,
            dto_types: false,
        })
        .expect("build");

        let out_dir = dir.join("web-runtime");
        assert!(out_dir.join("runtime-assets.json").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());
        assert!(out_dir.join("loaders.ts").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builds_hybrid_web_target() {
        let dir = temp_dir();
        for locale in ["en", "es"] {
            fs::create_dir_all(dir.join("locales").join(locale)).expect("locale");
        }
        fs::write(
            dir.join("locales/en/common.json"),
            r#"{"home":{"title":"Hi"},"cart":{"count":"{ $count :number } items"}}"#,
        )
        .expect("write en");
        fs::write(
            dir.join("locales/es/common.json"),
            r#"{"home":{"title":"Hola"},"cart":{"count":"{ $count :number } articulos"}}"#,
        )
        .expect("write es");
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "hybrid"
mode = "hybrid"
framework = "vite"
out_dir = "web-hybrid"
locales = ["en", "es"]
namespaces = ["common"]
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path: config_path.clone(),
            target_name: "hybrid".to_string(),
            out_dir: None,
            check: false,
            dto_types: false,
        })
        .expect("build");

        let out_dir = dir.join("web-hybrid");
        assert!(out_dir.join("hybrid-assets.json").exists());
        assert!(out_dir.join("runtime-assets.json").exists());
        assert!(out_dir.join("messages/en/common.json").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());
        assert!(out_dir.join("types.ts").exists());
        assert!(out_dir.join("loaders.ts").exists());

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "hybrid".to_string(),
            out_dir: Some(out_dir),
            check: true,
            dto_types: false,
        })
        .expect("check");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builds_docusaurus_web_target() {
        let dir = temp_dir();
        for locale in ["en", "es"] {
            fs::create_dir_all(dir.join("locales").join(locale)).expect("locale");
        }
        fs::write(dir.join("id_salt.txt"), "salt").expect("salt");
        fs::create_dir_all(dir.join("docs")).expect("docs");
        fs::write(dir.join("docs/intro.mdx"), "# Intro\n").expect("doc");
        fs::create_dir_all(dir.join("locales/es/docs")).expect("docs es");
        fs::write(dir.join("locales/en/home.mf2"), "title = Docs\n").expect("home en");
        fs::write(dir.join("locales/es/home.mf2"), "title = Documentos\n").expect("home es");
        fs::write(
            dir.join("locales/es/docs/intro.mf2"),
            "content = ---\ntitle: Intro\n---\n# Intro ES\n",
        )
        .expect("docs es");

        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "docusaurus"
framework = "docusaurus"
out_dir = "generated"
docs_source_dir = "docs"
docusaurus_i18n_dir = "i18n"
locales = ["en", "es"]
namespaces = ["home", "docs"]
"#,
        )
        .expect("config");

        run_build_web(&BuildWebOptions {
            config_path,
            target_name: "docs".to_string(),
            out_dir: None,
            check: false,
            dto_types: false,
        })
        .expect("build");

        assert!(dir.join("generated/messages.ts").exists());
        assert!(dir.join("i18n/es/code.json").exists());
        assert!(!dir.join("locales/en/docs.mf2").exists());
        assert!(dir.join("locales/es/docs/intro.mf2").exists());
        assert!(
            dir.join("i18n/es/docusaurus-plugin-content-docs/current/intro.mdx")
                .exists()
        );

        fs::remove_dir_all(&dir).ok();
    }
}
