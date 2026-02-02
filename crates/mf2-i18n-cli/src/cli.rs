use std::path::PathBuf;

use thiserror::Error;

use crate::command_build::{run_build, BuildCommandError, BuildOptions};
use crate::command_extract::{run_extract, ExtractCommandError, ExtractOptions};
use crate::command_sign::{run_sign, SignCommandError, SignOptions};
use crate::command_pseudo::{run_pseudo, PseudoCommandError, PseudoOptions};
use crate::command_coverage::{run_coverage, CoverageCommandError, CoverageOptions};
use crate::command_validate::{run_validate, ValidateCommandError, ValidateOptions};

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
    Sign(#[from] SignCommandError),
    #[error(transparent)]
    Pseudo(#[from] PseudoCommandError),
    #[error(transparent)]
    Coverage(#[from] CoverageCommandError),
}

pub fn run() -> Result<(), CliAppError> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .ok_or_else(|| CliAppError::Usage(usage()))?;
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
        "build" => {
            let options = parse_build_options(args.collect())?;
            run_build(&options)?;
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
        _ => Err(CliAppError::Usage(usage())),
    }
}

fn parse_extract_options(args: Vec<String>) -> Result<ExtractOptions, CliAppError> {
    let mut project = None;
    let mut roots = Vec::new();
    let mut out_dir = PathBuf::from("i18n");
    let mut config_path = PathBuf::from("mf2-i18n.toml");
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
    "usage: mf2-i18n-cli extract --project <id> --root <path> [--root <path>...] --generated-at <rfc3339> [--out <dir>] [--config <path>]\n       mf2-i18n-cli validate --catalog <path> --id-map-hash <path> [--config <path>]\n       mf2-i18n-cli build --catalog <path> --id-map-hash <path> --release-id <id> --generated-at <rfc3339> [--out <dir>] [--config <path>]\n       mf2-i18n-cli sign --manifest <path> --key <path> --key-id <id> [--out <path>]\n       mf2-i18n-cli pseudo --locale <tag> --target <tag> [--out <dir>] [--config <path>]\n       mf2-i18n-cli coverage --catalog <path> --id-map-hash <path> [--out <path>] [--config <path>]".to_string()
}

fn parse_validate_options(args: Vec<String>) -> Result<ValidateOptions, CliAppError> {
    let mut catalog_path = None;
    let mut id_map_hash_path = None;
    let mut config_path = PathBuf::from("mf2-i18n.toml");
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
    let mut catalog_path = None;
    let mut id_map_hash_path = None;
    let mut release_id = None;
    let mut generated_at = None;
    let mut out_dir = PathBuf::from("i18n-build");
    let mut config_path = PathBuf::from("mf2-i18n.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--catalog" => catalog_path = Some(PathBuf::from(next_value("--catalog", &mut iter)?)),
            "--id-map-hash" => {
                id_map_hash_path = Some(PathBuf::from(next_value("--id-map-hash", &mut iter)?))
            }
            "--release-id" => release_id = Some(next_value("--release-id", &mut iter)?),
            "--generated-at" => generated_at = Some(next_value("--generated-at", &mut iter)?),
            "--out" => out_dir = PathBuf::from(next_value("--out", &mut iter)?),
            "--config" => config_path = PathBuf::from(next_value("--config", &mut iter)?),
            "--help" | "-h" => return Err(CliAppError::Usage(usage())),
            _ => return Err(CliAppError::Usage(usage())),
        }
    }
    let catalog_path = catalog_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    let id_map_hash_path = id_map_hash_path.ok_or_else(|| CliAppError::Usage(usage()))?;
    let release_id = release_id.ok_or_else(|| CliAppError::Usage(usage()))?;
    let generated_at = generated_at.ok_or_else(|| CliAppError::Usage(usage()))?;
    Ok(BuildOptions {
        catalog_path,
        id_map_hash_path,
        config_path,
        out_dir,
        release_id,
        generated_at,
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
            "--manifest" => manifest_path = Some(PathBuf::from(next_value("--manifest", &mut iter)?)),
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
    let mut config_path = PathBuf::from("mf2-i18n.toml");
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
    let mut config_path = PathBuf::from("mf2-i18n.toml");
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

#[cfg(test)]
mod tests {
    use super::{parse_build_options, parse_coverage_options, parse_extract_options, parse_pseudo_options, parse_sign_options, parse_validate_options};

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
            "--catalog".to_string(),
            "i18n.catalog.json".to_string(),
            "--id-map-hash".to_string(),
            "id_map_hash".to_string(),
            "--release-id".to_string(),
            "r1".to_string(),
            "--generated-at".to_string(),
            "2026-02-01T00:00:00Z".to_string(),
        ];
        let options = parse_build_options(args).expect("options");
        assert_eq!(options.release_id, "r1");
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
}
