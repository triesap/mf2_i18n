use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::web::{WebArtifactError, WebArtifactFile};

const DTO_ARTIFACT_DIR: &str = "dto";
const DTO_CONFIG_FILE: &str = "dto_bindgen.toml";
static DTO_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum DtoExportError {
    #[error(transparent)]
    Export(#[from] dto_bindgen::export::ExportError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    WebArtifact(#[from] WebArtifactError),
}

pub fn web_dto_artifact_files() -> Result<Vec<WebArtifactFile>, DtoExportError> {
    let temp_dir = DtoTempDir::new()?;
    let config_path = temp_dir.path().join(DTO_CONFIG_FILE);
    fs::write(&config_path, dto_config())?;

    dto_bindgen::export_types!(
        config = config_path.clone(),
        roots = [
            crate::model::ArgSpec,
            crate::model::ArgType,
            crate::catalog::Catalog,
            crate::catalog::CatalogFeatures,
            crate::catalog::CatalogMessage,
            crate::web_metadata::HybridMessageDelivery,
            crate::web_metadata::HybridMessagePlan,
            crate::web_metadata::HybridMessageRoute,
            crate::web_hybrid::HybridWebAssets,
            // The web DTO package owns build-manifest shapes only. Runtime
            // manifest DTOs stay out of this package until they have separate
            // target names or a separate output package.
            crate::manifest::Manifest,
            crate::manifest::ManifestSigning,
            crate::model::MessageSpec,
            crate::manifest::PackEntry,
            crate::web_runtime::RuntimeWebAssets,
            crate::web_runtime::RuntimeWebPackAsset,
            crate::catalog::SourceRef,
            crate::web_metadata::WebMessageArg,
            crate::web_metadata::WebMessageMetadata,
        ],
    )?;

    collect_artifact_files(&temp_dir.path().join(DTO_ARTIFACT_DIR))
}

fn collect_artifact_files(root: &Path) -> Result<Vec<WebArtifactFile>, DtoExportError> {
    let mut files = Vec::new();
    collect_paths(root, root, &mut files)?;
    files.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    Ok(files)
}

fn collect_paths(
    root: &Path,
    current: &Path,
    files: &mut Vec<WebArtifactFile>,
) -> Result<(), DtoExportError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_paths(root, &path, files)?;
        } else if path.is_file() {
            let relative_path = path
                .strip_prefix(root)
                .expect("generated path should remain under root");
            files.push(WebArtifactFile::new(
                Path::new(DTO_ARTIFACT_DIR).join(relative_path),
                fs::read(&path)?,
            )?);
        }
    }
    Ok(())
}

fn dto_config() -> &'static str {
    r#"[export]
out_dir = "dto"

[numeric]
large_int_policy = "json_number"

[typescript]
enabled = true
out_dir = "dto/ts"
emit = "ts"
module_resolution = "bundler"

[python]
enabled = false
"#
}

struct DtoTempDir {
    path: PathBuf,
}

impl DtoTempDir {
    fn new() -> Result<Self, std::io::Error> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let counter = DTO_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mf2_i18n_dto_export_{}_{}_{}",
            std::process::id(),
            nanos,
            counter
        ));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DtoTempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::web_dto_artifact_files;

    #[test]
    fn web_dto_artifacts_export_manifest_family_in_typescript_bundle() {
        let files = artifact_files();
        let paths = files
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"dto/ts/index.ts"));
        assert!(paths.contains(&"dto/ts/types.ts"));
        assert!(!paths.iter().any(|path| path.ends_with("/manifest.ts")));
        assert!(
            !paths
                .iter()
                .any(|path| path.ends_with("/manifest_signing.ts"))
        );
        assert!(!paths.iter().any(|path| path.ends_with("/pack_entry.ts")));

        let types = files
            .iter()
            .find(|(path, _)| path == "dto/ts/types.ts")
            .map(|(_, contents)| contents.as_str())
            .expect("types bundle");
        assert!(types.contains("export type Manifest ="));
        assert!(types.contains("export type ManifestSigning ="));
        assert!(types.contains("export type PackEntry ="));
    }

    #[test]
    fn web_dto_artifacts_do_not_emit_python_package() {
        let files = artifact_files();
        let paths = files
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>();

        assert!(
            paths
                .iter()
                .all(|path| !path.starts_with("dto/python/") && !path.ends_with(".py"))
        );
        assert!(!paths.iter().any(|path| path.ends_with("py.typed")));
    }

    fn artifact_files() -> Vec<(String, String)> {
        web_dto_artifact_files()
            .expect("dto artifacts")
            .into_iter()
            .map(|file| {
                (
                    file.relative_path().to_string_lossy().into_owned(),
                    String::from_utf8(file.bytes().to_vec()).expect("utf-8 dto artifact"),
                )
            })
            .collect()
    }
}
