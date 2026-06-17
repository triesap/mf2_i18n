use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{WebFramework, WebStalePolicy, WebTargetMode};
use crate::manifest::sha256_hex;

pub const WEB_ARTIFACT_MANIFEST_FILE: &str = "mf2-web-manifest.json";
pub const WEB_ARTIFACT_MANIFEST_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebArtifactFile {
    relative_path: PathBuf,
    bytes: Vec<u8>,
}

impl WebArtifactFile {
    pub fn new(
        relative_path: impl Into<PathBuf>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, WebArtifactError> {
        let relative_path = validate_relative_path(relative_path.into())?;
        Ok(Self {
            relative_path,
            bytes: bytes.into(),
        })
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebArtifactManifestInput {
    pub target: String,
    pub mode: WebTargetMode,
    pub framework: Option<WebFramework>,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub namespaces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebArtifactWriteOptions {
    out_dir: PathBuf,
    stale_policy: WebStalePolicy,
    check: bool,
}

impl WebArtifactWriteOptions {
    pub fn new(out_dir: impl Into<PathBuf>) -> Self {
        Self {
            out_dir: out_dir.into(),
            stale_policy: WebStalePolicy::Fail,
            check: false,
        }
    }

    pub fn with_stale_policy(mut self, stale_policy: WebStalePolicy) -> Self {
        self.stale_policy = stale_policy;
        self
    }

    pub fn with_check(mut self, check: bool) -> Self {
        self.check = check;
        self
    }

    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn stale_policy(&self) -> WebStalePolicy {
        self.stale_policy
    }

    pub fn check(&self) -> bool {
        self.check
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebArtifactManifest {
    pub schema: u32,
    pub target: String,
    pub mode: WebTargetMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<WebFramework>,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub namespaces: Vec<String>,
    pub generated_files: Vec<WebArtifactManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebArtifactManifestFile {
    pub path: String,
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebArtifactWriteOutput {
    manifest: WebArtifactManifest,
    manifest_path: PathBuf,
    files: Vec<PathBuf>,
}

impl WebArtifactWriteOutput {
    pub fn manifest(&self) -> &WebArtifactManifest {
        &self.manifest
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebArtifactCheckFailure {
    pub differences: Vec<String>,
}

#[derive(Debug, Error)]
pub enum WebArtifactError {
    #[error("web artifact path must be relative: {0}")]
    AbsolutePath(String),
    #[error("web artifact path must not contain parent traversal: {0}")]
    ParentTraversal(String),
    #[error("duplicate web artifact path: {0}")]
    DuplicatePath(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("web artifact check failed: {0:?}")]
    CheckFailed(Vec<String>),
    #[error("stale generated web artifacts: {0:?}")]
    StaleFiles(Vec<String>),
}

pub fn write_web_artifacts(
    options: &WebArtifactWriteOptions,
    input: &WebArtifactManifestInput,
    files: Vec<WebArtifactFile>,
) -> Result<WebArtifactWriteOutput, WebArtifactError> {
    let artifact_set = WebArtifactSet::new(input, files)?;
    if options.check() {
        check_artifacts(options.out_dir(), &artifact_set, options.stale_policy())?;
        return Ok(output_for(options.out_dir(), artifact_set));
    }

    let stale = stale_files(options.out_dir(), &artifact_set.expected_paths())?;
    match options.stale_policy() {
        WebStalePolicy::Fail if !stale.is_empty() => {
            return Err(WebArtifactError::StaleFiles(stale));
        }
        WebStalePolicy::Ignore | WebStalePolicy::Fail | WebStalePolicy::Delete => {}
    }

    let stage_dir = stage_dir_for(options.out_dir());
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir)?;
    }
    fs::create_dir_all(&stage_dir)?;
    write_artifact_set(&stage_dir, &artifact_set)?;

    if options.out_dir().exists() {
        fs::remove_dir_all(options.out_dir())?;
    }
    fs::rename(&stage_dir, options.out_dir())?;
    Ok(output_for(options.out_dir(), artifact_set))
}

struct WebArtifactSet {
    manifest: WebArtifactManifest,
    manifest_bytes: Vec<u8>,
    files: Vec<WebArtifactFile>,
}

impl WebArtifactSet {
    fn new(
        input: &WebArtifactManifestInput,
        mut files: Vec<WebArtifactFile>,
    ) -> Result<Self, WebArtifactError> {
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        validate_unique_paths(&files)?;
        let generated_files = files
            .iter()
            .map(|file| WebArtifactManifestFile {
                path: path_literal(file.relative_path()),
                hash: sha256_hex(file.bytes()),
                size: file.bytes().len() as u64,
            })
            .collect();
        let manifest = WebArtifactManifest {
            schema: WEB_ARTIFACT_MANIFEST_SCHEMA,
            target: input.target.clone(),
            mode: input.mode,
            framework: input.framework,
            default_locale: input.default_locale.clone(),
            supported_locales: sorted(input.supported_locales.clone()),
            namespaces: sorted(input.namespaces.clone()),
            generated_files,
        };
        let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        manifest_bytes.push(b'\n');
        Ok(Self {
            manifest,
            manifest_bytes,
            files,
        })
    }

    fn expected_paths(&self) -> BTreeSet<String> {
        let mut paths = self
            .files
            .iter()
            .map(|file| path_literal(file.relative_path()))
            .collect::<BTreeSet<_>>();
        paths.insert(WEB_ARTIFACT_MANIFEST_FILE.to_string());
        paths
    }
}

fn write_artifact_set(root: &Path, artifact_set: &WebArtifactSet) -> Result<(), WebArtifactError> {
    for file in &artifact_set.files {
        let path = root.join(file.relative_path());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, file.bytes())?;
    }
    fs::write(
        root.join(WEB_ARTIFACT_MANIFEST_FILE),
        &artifact_set.manifest_bytes,
    )?;
    Ok(())
}

fn output_for(out_dir: &Path, artifact_set: WebArtifactSet) -> WebArtifactWriteOutput {
    let files = artifact_set
        .files
        .iter()
        .map(|file| out_dir.join(file.relative_path()))
        .chain(std::iter::once(out_dir.join(WEB_ARTIFACT_MANIFEST_FILE)))
        .collect();
    WebArtifactWriteOutput {
        manifest: artifact_set.manifest,
        manifest_path: out_dir.join(WEB_ARTIFACT_MANIFEST_FILE),
        files,
    }
}

fn check_artifacts(
    out_dir: &Path,
    artifact_set: &WebArtifactSet,
    stale_policy: WebStalePolicy,
) -> Result<(), WebArtifactError> {
    let mut differences = Vec::new();
    for file in &artifact_set.files {
        let path = out_dir.join(file.relative_path());
        match fs::read(&path) {
            Ok(bytes) if bytes == file.bytes() => {}
            Ok(_) => differences.push(format!("changed:{}", path_literal(file.relative_path()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                differences.push(format!("missing:{}", path_literal(file.relative_path())));
            }
            Err(error) => return Err(WebArtifactError::Io(error)),
        }
    }

    let manifest_path = out_dir.join(WEB_ARTIFACT_MANIFEST_FILE);
    match fs::read(&manifest_path) {
        Ok(bytes) if bytes == artifact_set.manifest_bytes => {}
        Ok(_) => differences.push(format!("changed:{WEB_ARTIFACT_MANIFEST_FILE}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            differences.push(format!("missing:{WEB_ARTIFACT_MANIFEST_FILE}"));
        }
        Err(error) => return Err(WebArtifactError::Io(error)),
    }

    let stale = stale_files(out_dir, &artifact_set.expected_paths())?;
    if stale_policy == WebStalePolicy::Fail && !stale.is_empty() {
        differences.extend(stale.into_iter().map(|path| format!("stale:{path}")));
    }

    if differences.is_empty() {
        Ok(())
    } else {
        Err(WebArtifactError::CheckFailed(differences))
    }
}

fn stale_files(
    out_dir: &Path,
    expected_paths: &BTreeSet<String>,
) -> Result<Vec<String>, WebArtifactError> {
    let manifest_path = out_dir.join(WEB_ARTIFACT_MANIFEST_FILE);
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }
    let manifest: WebArtifactManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    let mut stale = Vec::new();
    for file in manifest.generated_files {
        if !expected_paths.contains(&file.path) {
            stale.push(file.path);
        }
    }
    stale.sort();
    Ok(stale)
}

fn validate_unique_paths(files: &[WebArtifactFile]) -> Result<(), WebArtifactError> {
    let mut paths = BTreeSet::new();
    for file in files {
        let path = path_literal(file.relative_path());
        if !paths.insert(path.clone()) {
            return Err(WebArtifactError::DuplicatePath(path));
        }
    }
    Ok(())
}

fn validate_relative_path(path: PathBuf) -> Result<PathBuf, WebArtifactError> {
    if path.is_absolute() {
        return Err(WebArtifactError::AbsolutePath(path.display().to_string()));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                return Err(WebArtifactError::ParentTraversal(
                    path.display().to_string(),
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(WebArtifactError::AbsolutePath(path.display().to_string()));
            }
        }
    }
    Ok(path)
}

fn stage_dir_for(out_dir: &Path) -> PathBuf {
    let parent = out_dir.parent().unwrap_or_else(|| Path::new("."));
    let name = out_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("mf2-web-output");
    parent.join(format!(".{name}.mf2-stage-{}", std::process::id()))
}

fn path_literal(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            Component::CurDir => None,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

#[cfg(test)]
mod tests {
    use super::{
        WEB_ARTIFACT_MANIFEST_FILE, WebArtifactError, WebArtifactFile, WebArtifactManifestInput,
        WebArtifactWriteOptions, write_web_artifacts,
    };
    use crate::config::{WebFramework, WebStalePolicy, WebTargetMode};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_web_artifacts_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn input() -> WebArtifactManifestInput {
        WebArtifactManifestInput {
            target: "docs".to_string(),
            mode: WebTargetMode::PlainJson,
            framework: Some(WebFramework::Docusaurus),
            default_locale: "en".to_string(),
            supported_locales: vec!["es".to_string(), "en".to_string()],
            namespaces: vec!["home".to_string()],
        }
    }

    fn files() -> Vec<WebArtifactFile> {
        vec![
            WebArtifactFile::new("messages/en/home.json", b"{\"title\":\"Hi\"}\n".to_vec())
                .expect("file"),
            WebArtifactFile::new("index.ts", b"export {}\n".to_vec()).expect("file"),
        ]
    }

    #[test]
    fn writes_manifest_and_files() {
        let root = temp_dir("write");
        let out_dir = root.join("generated");
        let output =
            write_web_artifacts(&WebArtifactWriteOptions::new(&out_dir), &input(), files())
                .expect("write");

        assert_eq!(output.manifest().target, "docs");
        assert_eq!(
            output.manifest().supported_locales,
            ["en".to_string(), "es".to_string()]
        );
        assert!(out_dir.join("messages/en/home.json").exists());
        assert!(out_dir.join(WEB_ARTIFACT_MANIFEST_FILE).exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn check_mode_accepts_current_outputs() {
        let root = temp_dir("check_ok");
        let out_dir = root.join("generated");
        write_web_artifacts(&WebArtifactWriteOptions::new(&out_dir), &input(), files())
            .expect("write");

        let output = write_web_artifacts(
            &WebArtifactWriteOptions::new(&out_dir).with_check(true),
            &input(),
            files(),
        )
        .expect("check");

        assert_eq!(
            output.manifest_path(),
            out_dir.join(WEB_ARTIFACT_MANIFEST_FILE)
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn check_mode_reports_drift() {
        let root = temp_dir("check_drift");
        let out_dir = root.join("generated");
        write_web_artifacts(&WebArtifactWriteOptions::new(&out_dir), &input(), files())
            .expect("write");
        fs::write(out_dir.join("index.ts"), "changed\n").expect("drift");

        let err = write_web_artifacts(
            &WebArtifactWriteOptions::new(&out_dir).with_check(true),
            &input(),
            files(),
        )
        .expect_err("drift");

        assert!(
            matches!(err, WebArtifactError::CheckFailed(values) if values.contains(&"changed:index.ts".to_string()))
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn stale_policy_fail_rejects_removed_generated_file() {
        let root = temp_dir("stale");
        let out_dir = root.join("generated");
        write_web_artifacts(&WebArtifactWriteOptions::new(&out_dir), &input(), files())
            .expect("write");

        let err = write_web_artifacts(
            &WebArtifactWriteOptions::new(&out_dir).with_stale_policy(WebStalePolicy::Fail),
            &input(),
            vec![WebArtifactFile::new("index.ts", b"export {}\n".to_vec()).expect("file")],
        )
        .expect_err("stale");

        assert!(
            matches!(err, WebArtifactError::StaleFiles(values) if values.contains(&"messages/en/home.json".to_string()))
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_parent_traversal_paths() {
        let err = WebArtifactFile::new("../bad.json", Vec::new()).expect_err("bad path");
        assert!(matches!(err, WebArtifactError::ParentTraversal(_)));
    }
}
