use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::catalog_builder::{build_catalog, BuildOutput, CatalogBuildError};
use crate::extract::{extract_messages, ExtractError, ExtractedMessage};

#[derive(Debug, Error)]
pub enum ExtractPipelineError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Extract(#[from] ExtractError),
    #[error(transparent)]
    Build(#[from] CatalogBuildError),
    #[error("conflicting argument specs for key {0}")]
    ConflictingArgs(String),
}

pub fn collect_rust_files(roots: &[PathBuf]) -> Result<Vec<PathBuf>, ExtractPipelineError> {
    let mut files = Vec::new();
    for root in roots {
        collect_rust_files_inner(root, &mut files)?;
    }
    Ok(files)
}

pub fn extract_from_sources(
    roots: &[PathBuf],
    project: &str,
    default_locale: &str,
    generated_at: &str,
    salt: &[u8],
) -> Result<BuildOutput, ExtractPipelineError> {
    let files = collect_rust_files(roots)?;
    extract_from_files(&files, project, default_locale, generated_at, salt)
}

pub fn extract_from_files(
    files: &[PathBuf],
    project: &str,
    default_locale: &str,
    generated_at: &str,
    salt: &[u8],
) -> Result<BuildOutput, ExtractPipelineError> {
    let mut by_key: BTreeMap<String, ExtractedMessage> = BTreeMap::new();
    for path in files {
        let contents = fs::read_to_string(path)?;
        let extracted = extract_messages(&contents)?;
        for message in extracted {
            if let Some(existing) = by_key.get(&message.key) {
                if existing.args != message.args {
                    return Err(ExtractPipelineError::ConflictingArgs(message.key));
                }
                continue;
            }
            by_key.insert(message.key.clone(), message);
        }
    }
    let messages: Vec<ExtractedMessage> = by_key.into_values().collect();
    Ok(build_catalog(
        &messages,
        project,
        default_locale,
        generated_at,
        salt,
    )?)
}

fn collect_rust_files_inner(
    root: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ExtractPipelineError> {
    if root.is_file() {
        if root.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(root.to_path_buf());
        }
        return Ok(());
    }
    if should_skip_dir(root) {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files_inner(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git") | Some("target") | Some("node_modules")
    )
}

#[cfg(test)]
mod tests {
    use super::extract_from_files;
    use crate::id_map::derive_message_id;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_extract_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn extracts_from_multiple_files() {
        let dir = temp_dir();
        let file_a = dir.join("a.rs");
        let file_b = dir.join("b.rs");
        fs::write(&file_a, "let _ = t!(\"home.title\");").expect("write");
        fs::write(&file_b, "let _ = t!(\"cart.items\");").expect("write");

        let output = extract_from_files(
            &[file_a, file_b],
            "demo",
            "en",
            "2026-02-01T00:00:00Z",
            b"salt",
        )
        .expect("extract");

        let expected = derive_message_id("home.title", b"salt");
        assert!(output
            .catalog
            .messages
            .iter()
            .any(|message| message.id == u32::from(expected)));

        fs::remove_dir_all(&dir).ok();
    }
}
