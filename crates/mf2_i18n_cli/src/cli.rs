use std::path::PathBuf;

use thiserror::Error;

use crate::command_build::{BuildCommandError, BuildOptions, run_build};
use crate::command_build_web::{BuildWebCommandError, BuildWebOptions, run_build_web};
use crate::command_coverage::{CoverageCommandError, CoverageOptions, run_coverage};
use crate::command_detect::{DetectOptions, run_detect};
use crate::command_doctor::{DoctorCommandError, DoctorOptions, run_doctor};
use crate::command_export_web_json::{
    ExportWebJsonCommandError, ExportWebJsonOptions, run_export_web_json,
};
use crate::command_extract::{ExtractCommandError, ExtractOptions, run_extract};
use crate::command_init::{InitCommandError, InitOptions, run_init};
use crate::command_pseudo::{PseudoCommandError, PseudoOptions, run_pseudo};
use crate::command_scaffold::{ScaffoldCommandError, ScaffoldMissingOptions, run_scaffold_missing};
use crate::command_sign::{SignCommandError, SignOptions, run_sign};
use crate::command_validate::{ValidateCommandError, ValidateOptions, run_validate};
use mf2_i18n_build::{Diagnostic, DiagnosticReport, WebJsonMode};

#[derive(Debug, Error)]
pub enum CliAppError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Extract(#[from] ExtractCommandError),
    #[error(transparent)]
    Validate(#[from] ValidateCommandError),
    #[error(transparent)]
    Build(#[from] BuildCommandError),
    #[error(transparent)]
    BuildWeb(#[from] BuildWebCommandError),
    #[error(transparent)]
    Sign(#[from] SignCommandError),
    #[error(transparent)]
    Pseudo(#[from] PseudoCommandError),
    #[error(transparent)]
    Coverage(#[from] CoverageCommandError),
    #[error(transparent)]
    ExportWebJson(#[from] ExportWebJsonCommandError),
    #[error(transparent)]
    Doctor(#[from] DoctorCommandError),
    #[error(transparent)]
    Init(#[from] InitCommandError),
    #[error(transparent)]
    Scaffold(#[from] ScaffoldCommandError),
}

pub fn run_args(raw_args: Vec<String>) -> Result<(), CliAppError> {
    let (args, _) = strip_json_flag(raw_args);
    let mut args = args.into_iter();
    let command = args.next().ok_or_else(|| CliAppError::Usage(usage()))?;
    match command.as_str() {
        "extract" => {
            let options = parse_extract_options(args.collect())?;
            run_extract(&options)?;
            Ok(())
        }
        "validate" => {
            let options = parse_validate_options(args.collect())?;
            match run_validate(&options) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.into()),
            }
        }
        "build-native" => {
            let options = parse_build_options(args.collect())?;
            run_build(&options)?;
            Ok(())
        }
        "build" => {
            let options = parse_build_web_options(args.collect())?;
            run_build_web(&options)?;
            Ok(())
        }
        "check" => {
            let mut options = parse_build_web_options(args.collect())?;
            options.check = true;
            run_build_web(&options)?;
            Ok(())
        }
        "build-web" => {
            let options = parse_build_web_options(args.collect())?;
            run_build_web(&options)?;
            Ok(())
        }
        "doctor" => {
            let options = parse_doctor_options(args.collect())?;
            run_doctor(&options)?;
            Ok(())
        }
        "detect" => {
            let options = parse_detect_options(args.collect())?;
            let _ = run_detect(&options);
            Ok(())
        }
        "init" => {
            let options = parse_init_options(args.collect())?;
            run_init(&options)?;
            Ok(())
        }
        "scaffold" => {
            let options = parse_scaffold_missing_options(args.collect())?;
            run_scaffold_missing(&options)?;
            Ok(())
        }
        "sign" => {
            let options = parse_sign_options(args.collect())?;
            run_sign(&options)?;
            Ok(())
        }
        "pseudo" => {
            let options = parse_pseudo_options(args.collect())?;
            run_pseudo(&options)?;
            Ok(())
        }
        "coverage" => {
            let options = parse_coverage_options(args.collect())?;
            run_coverage(&options)?;
            Ok(())
        }
        "export-web-json" => {
            let options = parse_export_web_json_options(args.collect())?;
            run_export_web_json(&options)?;
            Ok(())
        }
        _ => Err(CliAppError::Usage(usage())),
    }
}

pub fn args_request_json(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--json")
}

pub fn error_diagnostic_report(error: &CliAppError) -> DiagnosticReport {
    DiagnosticReport::new(vec![error_diagnostic(error)])
}

pub fn render_error_json(error: &CliAppError) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&error_diagnostic_report(error))
}

fn error_diagnostic(error: &CliAppError) -> Diagnostic {
    match error {
        CliAppError::Usage(message) => Diagnostic::new("MF2CLI_USAGE", message.clone())
            .with_suggestion("Run mf2_i18n_cli --help."),
        CliAppError::Extract(_) => command_error("MF2CLI_EXTRACT", error),
        CliAppError::Validate(_) => command_error("MF2CLI_VALIDATE", error),
        CliAppError::Build(_) => command_error("MF2CLI_BUILD", error),
        CliAppError::BuildWeb(_) => command_error("MF2CLI_BUILD_WEB", error),
        CliAppError::Sign(_) => command_error("MF2CLI_SIGN", error),
        CliAppError::Pseudo(_) => command_error("MF2CLI_PSEUDO", error),
        CliAppError::Coverage(_) => command_error("MF2CLI_COVERAGE", error),
        CliAppError::ExportWebJson(_) => command_error("MF2CLI_EXPORT_WEB_JSON", error),
        CliAppError::Doctor(_) => command_error("MF2CLI_DOCTOR", error),
        CliAppError::Init(_) => command_error("MF2CLI_INIT", error),
        CliAppError::Scaffold(_) => command_error("MF2CLI_SCAFFOLD", error),
    }
}

fn command_error(code: &str, error: &CliAppError) -> Diagnostic {
    Diagnostic::new(code, error.to_string()).with_suggestion("Run the command again with --help.")
}

fn strip_json_flag(args: Vec<String>) -> (Vec<String>, bool) {
    let mut json = false;
    let filtered = args
        .into_iter()
        .filter(|arg| {
            if arg == "--json" {
                json = true;
                false
            } else {
                true
            }
        })
        .collect();
    (filtered, json)
}

fn parse_extract_options(args: Vec<String>) -> Result<ExtractOptions, CliAppError> {
    let mut project = None;
    let mut roots = Vec::new();
    let mut out_dir = PathBuf::from("i18n");
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut generated_at = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--project" => project = Some(next_value("--project", &mut iter)?),
            "--root" => roots.push(PathBuf::from(next_value("--root", &mut iter)?)),
            "--out" => out_dir = PathBuf::from(next_value("--out", &mut iter)?),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--generated-at" => generated_at = Some(next_value("--generated-at", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }

    let project = project.ok_or_else(|| CliAppError::Usage(usage()))?;
    let generated_at = generated_at.ok_or_else(|| CliAppError::Usage(usage()))?;
    if roots.is_empty() {
        return Err(CliAppError::Usage(usage()));
    }

    Ok(ExtractOptions {
        project,
        roots,
        out_dir,
        config_path,
        generated_at,
    })
}

fn next_value(flag: &str, iter: &mut impl Iterator<Item = String>) -> Result<String, CliAppError> {
    iter.next()
        .ok_or_else(|| CliAppError::Usage(format!("{flag} requires a value\n\n{}", usage())))
}

fn usage() -> String {
    let dto_flag = if cfg!(feature = "dto-bindgen") {
        " [--dto-types]"
    } else {
        ""
    };
    format!(
        "usage: mf2_i18n_cli init [--preset <name>] [--source-layout <name>] [--target <name>] [--config <path>] [--json]\n       mf2_i18n_cli detect [--cwd <path>] [--json]\n       mf2_i18n_cli build --target <name> [--out <dir>] [--config <path>] [--json]\n       mf2_i18n_cli check --target <name> [--out <dir>] [--config <path>] [--json]\n       mf2_i18n_cli scaffold missing --source <name> --locale <locale> [--config <path>] [--json]\n       mf2_i18n_cli doctor [--config <path>] [--json]\n       mf2_i18n_cli build-web --target <name> [--out <dir>] [--config <path>] [--check]{dto_flag} [--json]\n       mf2_i18n_cli build-native --release-id <id> --generated-at <rfc3339> [--out <dir>] [--config <path>] [--json]\n       mf2_i18n_cli extract --project <id> --root <path> [--root <path>...] --generated-at <rfc3339> [--out <dir>] [--config <path>] [--json]\n       mf2_i18n_cli validate --catalog <path> --id-map-hash <path> [--config <path>] [--json]\n       mf2_i18n_cli sign --manifest <path> --key <path> --key-id <id> [--out <path>] [--json]\n       mf2_i18n_cli pseudo --locale <tag> --target <tag> [--out <dir>] [--config <path>] [--json]\n       mf2_i18n_cli coverage --catalog <path> --id-map-hash <path> [--out <path>] [--config <path>] [--json]\n       mf2_i18n_cli export-web-json [--mode plain] [--out <dir>] [--config <path>] [--json]"
    )
}

fn parse_validate_options(args: Vec<String>) -> Result<ValidateOptions, CliAppError> {
    let mut catalog_path = None;
    let mut id_map_hash_path = None;
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--catalog" => catalog_path = Some(PathBuf::from(next_value("--catalog", &mut iter)?)),
            "--id-map-hash" => {
                id_map_hash_path = Some(PathBuf::from(next_value("--id-map-hash", &mut iter)?))
            }
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let catalog_path = catalog_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    let id_map_hash_path = id_map_hash_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    Ok(ValidateOptions {
        catalog_path,
        id_map_hash_path,
        config_path,
    })
}

fn parse_build_options(args: Vec<String>) -> Result<BuildOptions, CliAppError> {
    let mut release_id = None;
    let mut generated_at = None;
    let mut out_dir = PathBuf::from("i18n-build");
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--release-id" => release_id = Some(next_value("--release-id", &mut iter)?),
            "--generated-at" => generated_at = Some(next_value("--generated-at", &mut iter)?),
            "--out" => out_dir = PathBuf::from(next_value("--out", &mut iter)?),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let release_id = release_id.ok_or_else(|| CliAppError::Usage(usage()))?;
    let generated_at = generated_at.ok_or_else(|| CliAppError::Usage(usage()))?;
    Ok(BuildOptions {
        config_path,
        out_dir,
        release_id,
        generated_at,
    })
}

fn parse_build_web_options(args: Vec<String>) -> Result<BuildWebOptions, CliAppError> {
    let mut target_name = None;
    let mut out_dir = None;
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut check = false;
    let mut dto_types = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--target" => target_name = Some(next_value("--target", &mut iter)?),
            "--out" => out_dir = Some(PathBuf::from(next_value("--out", &mut iter)?)),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--check" => check = true,
            "--dto-types" if cfg!(feature = "dto-bindgen") => dto_types = true,
            "--dto-types" => {
                return Err(CliAppError::Usage(format!(
                    "--dto-types requires building mf2_i18n_cli with the dto-bindgen feature\n\n{}",
                    usage()
                )));
            }
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let target_name = target_name.ok_or_else(|| CliAppError::Usage(usage()))?;
    Ok(BuildWebOptions {
        config_path,
        target_name,
        out_dir,
        check,
        dto_types,
    })
}

fn parse_doctor_options(args: Vec<String>) -> Result<DoctorOptions, CliAppError> {
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    Ok(DoctorOptions { config_path })
}

fn parse_detect_options(args: Vec<String>) -> Result<DetectOptions, CliAppError> {
    let mut cwd = PathBuf::from(".");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--cwd" => cwd = PathBuf::from(next_value("--cwd", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    Ok(DetectOptions { cwd })
}

fn parse_init_options(args: Vec<String>) -> Result<InitOptions, CliAppError> {
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut preset = "vite".to_string();
    let mut source_layout = "locale-namespace".to_string();
    let mut target_name = "web".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--preset" => preset = next_value("--preset", &mut iter)?,
            "--source-layout" => source_layout = next_value("--source-layout", &mut iter)?,
            "--target" => target_name = next_value("--target", &mut iter)?,
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    Ok(InitOptions {
        config_path,
        preset,
        source_layout,
        target_name,
    })
}

fn parse_scaffold_missing_options(
    args: Vec<String>,
) -> Result<ScaffoldMissingOptions, CliAppError> {
    let mut iter = args.into_iter();
    match iter.next().as_deref() {
        Some("missing") => {}
        _ => return Err(CliAppError::Usage(usage())),
    }
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut source_name = None;
    let mut locale = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--source" => source_name = Some(next_value("--source", &mut iter)?),
            "--locale" => locale = Some(next_value("--locale", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    Ok(ScaffoldMissingOptions {
        config_path,
        source_name: source_name.ok_or_else(|| CliAppError::Usage(usage()))?,
        locale: locale.ok_or_else(|| CliAppError::Usage(usage()))?,
    })
}

fn parse_sign_options(args: Vec<String>) -> Result<SignOptions, CliAppError> {
    let mut manifest_path = None;
    let mut key_path = None;
    let mut key_id = None;
    let mut out_path = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest_path = Some(PathBuf::from(next_value("--manifest", &mut iter)?))
            }
            "--key" => key_path = Some(PathBuf::from(next_value("--key", &mut iter)?)),
            "--key-id" => key_id = Some(next_value("--key-id", &mut iter)?),
            "--out" => out_path = Some(PathBuf::from(next_value("--out", &mut iter)?)),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    let key_path = key_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    let key_id = key_id.ok_or_else(|| CliAppError::Usage(usage()))?;
    Ok(SignOptions {
        manifest_path,
        key_path,
        key_id,
        out_path,
    })
}

fn parse_pseudo_options(args: Vec<String>) -> Result<PseudoOptions, CliAppError> {
    let mut locale = None;
    let mut target = None;
    let mut out_dir = PathBuf::from("locales");
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--locale" => locale = Some(next_value("--locale", &mut iter)?),
            "--target" => target = Some(next_value("--target", &mut iter)?),
            "--out" => out_dir = PathBuf::from(next_value("--out", &mut iter)?),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let locale = locale.ok_or_else(|| CliAppError::Usage(usage()))?;
    let target = target.unwrap_or_else(|| "en-xa".to_string());
    Ok(PseudoOptions {
        locale,
        target,
        out_dir,
        config_path,
    })
}

fn parse_coverage_options(args: Vec<String>) -> Result<CoverageOptions, CliAppError> {
    let mut catalog_path = None;
    let mut id_map_hash_path = None;
    let mut out_path = PathBuf::from("coverage.json");
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--catalog" => catalog_path = Some(PathBuf::from(next_value("--catalog", &mut iter)?)),
            "--id-map-hash" => {
                id_map_hash_path = Some(PathBuf::from(next_value("--id-map-hash", &mut iter)?))
            }
            "--out" => out_path = PathBuf::from(next_value("--out", &mut iter)?),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let catalog_path = catalog_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    let id_map_hash_path = id_map_hash_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    Ok(CoverageOptions {
        catalog_path,
        id_map_hash_path,
        out_path,
        config_path,
    })
}

fn parse_export_web_json_options(args: Vec<String>) -> Result<ExportWebJsonOptions, CliAppError> {
    let mut out_dir = PathBuf::from("web-json");
    let mut config_path = PathBuf::from("mf2_i18n.toml");
    let mut mode = WebJsonMode::Plain;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => out_dir = PathBuf::from(next_value("--out", &mut iter)?),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--mode" => {
                mode = next_value("--mode", &mut iter)?
                    .parse()
                    .map_err(|_| CliAppError::Usage(usage()))?
            }
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    Ok(ExportWebJsonOptions {
        config_path,
        out_dir,
        mode,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CliAppError, parse_build_options, parse_build_web_options, parse_coverage_options,
        parse_detect_options, parse_doctor_options, parse_export_web_json_options,
        parse_extract_options, parse_init_options, parse_pseudo_options,
        parse_scaffold_missing_options, parse_sign_options, parse_validate_options,
        render_error_json, run_args, strip_json_flag,
    };
    use mf2_i18n_build::WebJsonMode;

    #[test]
    fn parses_extract_options() {
        let args = vec![
            "--project".to_string(),
            "demo".to_string(),
            "--root".to_string(),
            "src".to_string(),
            "--generated-at".to_string(),
            "2026-02-01T00:00:00Z".to_string(),
        ];
        let options = parse_extract_options(args).expect("options");
        assert_eq!(options.project, "demo");
        assert_eq!(options.roots.len(), 1);
    }

    #[test]
    fn parses_validate_options() {
        let args = vec![
            "--catalog".to_string(),
            "i18n.catalog.json".to_string(),
            "--id-map-hash".to_string(),
            "id_map_hash".to_string(),
        ];
        let options = parse_validate_options(args).expect("options");
        assert!(options.catalog_path.ends_with("i18n.catalog.json"));
    }

    #[test]
    fn parses_build_options() {
        let args = vec![
            "--release-id".to_string(),
            "r1".to_string(),
            "--generated-at".to_string(),
            "2026-02-01T00:00:00Z".to_string(),
        ];
        let options = parse_build_options(args).expect("options");
        assert_eq!(options.release_id, "r1");
    }

    #[test]
    fn parses_build_web_options() {
        let args = vec![
            "--target".to_string(),
            "docs".to_string(),
            "--out".to_string(),
            "web-generated".to_string(),
            "--config".to_string(),
            "mf2_i18n.toml".to_string(),
            "--check".to_string(),
        ];
        let options = parse_build_web_options(args).expect("options");
        assert_eq!(options.target_name, "docs");
        assert!(options.out_dir.expect("out").ends_with("web-generated"));
        assert!(options.check);
        assert!(!options.dto_types);
    }

    #[cfg(feature = "dto-bindgen")]
    #[test]
    fn parses_build_web_dto_types_when_feature_enabled() {
        let args = vec![
            "--target".to_string(),
            "docs".to_string(),
            "--dto-types".to_string(),
        ];
        let options = parse_build_web_options(args).expect("options");
        assert_eq!(options.target_name, "docs");
        assert!(options.dto_types);
    }

    #[cfg(not(feature = "dto-bindgen"))]
    #[test]
    fn rejects_build_web_dto_types_without_feature() {
        let err = parse_build_web_options(vec![
            "--target".to_string(),
            "docs".to_string(),
            "--dto-types".to_string(),
        ])
        .expect_err("feature error");

        let CliAppError::Usage(message) = err else {
            panic!("expected usage error");
        };
        assert!(message.contains("requires building mf2_i18n_cli with the dto-bindgen feature"));
    }

    #[test]
    fn parses_doctor_options() {
        let options = parse_doctor_options(vec![
            "--config".to_string(),
            "config/mf2_i18n.toml".to_string(),
        ])
        .expect("options");

        assert!(options.config_path.ends_with("config/mf2_i18n.toml"));
    }

    #[test]
    fn parses_detect_options() {
        let options = parse_detect_options(vec!["--cwd".to_string(), "examples/site".to_string()])
            .expect("options");

        assert!(options.cwd.ends_with("examples/site"));
    }

    #[test]
    fn parses_init_options() {
        let options = parse_init_options(vec![
            "--preset".to_string(),
            "react".to_string(),
            "--source-layout".to_string(),
            "namespace-locale".to_string(),
            "--target".to_string(),
            "app".to_string(),
        ])
        .expect("options");

        assert_eq!(options.preset, "react");
        assert_eq!(options.source_layout, "namespace-locale");
        assert_eq!(options.target_name, "app");
    }

    #[test]
    fn parses_scaffold_missing_options() {
        let options = parse_scaffold_missing_options(vec![
            "missing".to_string(),
            "--source".to_string(),
            "site".to_string(),
            "--locale".to_string(),
            "es".to_string(),
        ])
        .expect("options");

        assert_eq!(options.source_name, "site");
        assert_eq!(options.locale, "es");
    }

    #[test]
    fn parses_sign_options() {
        let args = vec![
            "--manifest".to_string(),
            "manifest.json".to_string(),
            "--key".to_string(),
            "signing.key".to_string(),
            "--key-id".to_string(),
            "key-1".to_string(),
        ];
        let options = parse_sign_options(args).expect("options");
        assert!(options.manifest_path.ends_with("manifest.json"));
    }

    #[test]
    fn parses_pseudo_options() {
        let args = vec![
            "--locale".to_string(),
            "en".to_string(),
            "--target".to_string(),
            "en-xa".to_string(),
        ];
        let options = parse_pseudo_options(args).expect("options");
        assert_eq!(options.locale, "en");
        assert_eq!(options.target, "en-xa");
    }

    #[test]
    fn parses_coverage_options() {
        let args = vec![
            "--catalog".to_string(),
            "catalog.json".to_string(),
            "--id-map-hash".to_string(),
            "id_map_hash".to_string(),
        ];
        let options = parse_coverage_options(args).expect("options");
        assert!(options.out_path.ends_with("coverage.json"));
    }

    #[test]
    fn parses_export_web_json_options() {
        let args = vec![
            "--mode".to_string(),
            "plain".to_string(),
            "--out".to_string(),
            "web-json".to_string(),
        ];
        let options = parse_export_web_json_options(args).expect("options");
        assert_eq!(options.mode, WebJsonMode::Plain);
        assert!(options.out_dir.ends_with("web-json"));
    }

    #[test]
    fn strips_global_json_flag_before_command_parsing() {
        let (args, json) = strip_json_flag(vec![
            "build-web".to_string(),
            "--target".to_string(),
            "docs".to_string(),
            "--json".to_string(),
        ]);

        assert!(json);
        assert_eq!(
            args,
            [
                "build-web".to_string(),
                "--target".to_string(),
                "docs".to_string()
            ]
        );
    }

    #[test]
    fn renders_usage_errors_as_diagnostic_json() {
        let payload = render_error_json(&CliAppError::Usage("bad command".to_string()))
            .expect("json payload");
        let value: serde_json::Value = serde_json::from_str(&payload).expect("json");

        assert_eq!(value["diagnostics"][0]["code"], "MF2CLI_USAGE");
        assert_eq!(value["diagnostics"][0]["severity"], "error");
        assert_eq!(value["diagnostics"][0]["message"], "bad command");
        assert_eq!(
            value["diagnostics"][0]["suggestions"][0],
            "Run mf2_i18n_cli --help."
        );
    }

    #[test]
    fn public_check_requires_target() {
        let err = run_args(vec!["check".to_string()]).expect_err("usage");

        assert!(matches!(err, CliAppError::Usage(_)));
    }
}
