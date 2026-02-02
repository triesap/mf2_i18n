use std::path::PathBuf;

use thiserror::Error;

use crate::command_extract::{run_extract, ExtractCommandError, ExtractOptions};
use crate::command_validate::{run_validate, ValidateCommandError, ValidateOptions};

#[derive(Debug, Error)]
pub enum CliAppError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Extract(#[from] ExtractCommandError),
    #[error(transparent)]
    Validate(#[from] ValidateCommandError),
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
    "usage: mf2-i18n-cli extract --project <id> --root <path> [--root <path>...] --generated-at <rfc3339> [--out <dir>] [--config <path>]\n       mf2-i18n-cli validate --catalog <path> --id-map-hash <path> [--config <path>]".to_string()
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

#[cfg(test)]
mod tests {
    use super::{parse_extract_options, parse_validate_options};

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
}
