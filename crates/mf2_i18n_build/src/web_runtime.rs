use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::Manifest;
use crate::project::{ProjectError, ProjectLayout, resolve_config_relative_path};
use crate::project_catalogs::{ProjectCatalogError, load_project_catalogs};
use crate::project_pipeline::{
    NativeModuleBuildError, ProjectRuntimeBuildOptions, build_project_runtime_artifacts,
};
use crate::web::{
    WebArtifactError, WebArtifactFile, WebArtifactManifestInput, WebArtifactWriteOptions,
    WebArtifactWriteOutput, write_web_artifacts,
};
use crate::web_metadata::{
    WebMessageMetadata, WebMessageMetadataError, collect_web_message_metadata,
    render_web_typescript_metadata_types,
};
use crate::{WebStalePolicy, WebTargetConfigError, WebTargetMode};

const DEFAULT_RUNTIME_GENERATED_AT: &str = "1970-01-01T00:00:00Z";
const RUNTIME_ASSETS_FILE: &str = "runtime-assets.json";
const MANIFEST_FILE: &str = "manifest.json";
const PLATFORM_BUNDLE_FILE: &str = "platform-bundle.json";
const ID_MAP_JSON_FILE: &str = "id-map.json";
const ID_MAP_HASH_FILE: &str = "id-map.sha256";

#[derive(Debug, Clone)]
pub struct RuntimeWebBuildOptions {
    config_path: PathBuf,
    target_name: String,
    out_dir_override: Option<PathBuf>,
    check: bool,
}

impl RuntimeWebBuildOptions {
    pub fn new(config_path: impl Into<PathBuf>, target_name: impl Into<String>) -> Self {
        Self {
            config_path: config_path.into(),
            target_name: target_name.into(),
            out_dir_override: None,
            check: false,
        }
    }

    pub fn with_out_dir(mut self, out_dir: impl Into<PathBuf>) -> Self {
        self.out_dir_override = Some(out_dir.into());
        self
    }

    pub fn with_check(mut self, check: bool) -> Self {
        self.check = check;
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn target_name(&self) -> &str {
        &self.target_name
    }

    pub fn out_dir_override(&self) -> Option<&Path> {
        self.out_dir_override.as_deref()
    }

    pub fn check(&self) -> bool {
        self.check
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeWebBuildOutput {
    asset_output: WebArtifactWriteOutput,
    code_output: Option<WebArtifactWriteOutput>,
    runtime_assets: RuntimeWebAssets,
    default_locale: String,
    supported_locales: Vec<String>,
    namespaces: Vec<String>,
    message_metadata: Vec<WebMessageMetadata>,
}

impl RuntimeWebBuildOutput {
    pub fn asset_output(&self) -> &WebArtifactWriteOutput {
        &self.asset_output
    }

    pub fn code_output(&self) -> Option<&WebArtifactWriteOutput> {
        self.code_output.as_ref()
    }

    pub fn runtime_assets(&self) -> &RuntimeWebAssets {
        &self.runtime_assets
    }

    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    pub fn supported_locales(&self) -> &[String] {
        &self.supported_locales
    }

    pub fn namespaces(&self) -> &[String] {
        &self.namespaces
    }

    pub fn message_metadata(&self) -> &[WebMessageMetadata] {
        &self.message_metadata
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWebAssets {
    pub schema: u32,
    pub target: String,
    pub mode: String,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub manifest: String,
    pub platform_bundle: String,
    pub id_map: String,
    pub id_map_hash: String,
    pub metadata: String,
    pub packs: Vec<RuntimeWebPackAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWebPackAsset {
    pub locale: String,
    pub url: String,
    pub kind: String,
    pub hash: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Error)]
pub enum RuntimeWebBuildError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] WebTargetConfigError),
    #[error(transparent)]
    Catalogs(#[from] ProjectCatalogError),
    #[error(transparent)]
    Runtime(#[from] NativeModuleBuildError),
    #[error(transparent)]
    Metadata(#[from] WebMessageMetadataError),
    #[error(transparent)]
    Artifacts(#[from] WebArtifactError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("missing web target {0}")]
    MissingTarget(String),
    #[error("web target {target} must use runtime mode")]
    UnsupportedTargetMode { target: String },
    #[error("runtime pack path must have a utf-8 file name: {0:?}")]
    InvalidPackPath(PathBuf),
}

pub fn build_runtime_web_target(
    options: &RuntimeWebBuildOptions,
) -> Result<RuntimeWebBuildOutput, RuntimeWebBuildError> {
    let project = ProjectLayout::load_or_default(options.config_path())?;
    project.config().validate_web_targets()?;
    let target = project
        .config()
        .web
        .target(options.target_name())
        .ok_or_else(|| RuntimeWebBuildError::MissingTarget(options.target_name().to_owned()))?
        .clone();
    if target.mode != WebTargetMode::Runtime {
        return Err(RuntimeWebBuildError::UnsupportedTargetMode {
            target: target.name.clone(),
        });
    }

    let loaded_catalogs = load_project_catalogs(&project)?;
    let default_locale = project.config().default_locale.clone();
    let default_catalog = loaded_catalogs
        .catalog(&default_locale)
        .ok_or_else(|| ProjectCatalogError::MissingDefaultLocale(default_locale.clone()))?;
    let namespace_filter = (!target.namespaces.is_empty()).then_some(target.namespaces.as_slice());
    let message_metadata =
        collect_web_message_metadata(&default_locale, default_catalog, namespace_filter)?;
    let namespaces = namespaces_from_metadata(&message_metadata);

    let temp_dir = RuntimeTempDir::new(&target.name)?;
    let runtime_output = build_project_runtime_artifacts(
        &ProjectRuntimeBuildOptions::new(
            options.config_path(),
            temp_dir.path(),
            target.release_id.as_deref().unwrap_or(&target.name),
            target
                .generated_at
                .as_deref()
                .unwrap_or(DEFAULT_RUNTIME_GENERATED_AT),
        )
        .with_locales(target.locales.clone())
        .with_namespaces(target.namespaces.clone()),
    )?;

    let runtime_assets =
        runtime_assets_from_manifest(&target.name, runtime_output.manifest_path())?;

    let asset_files = runtime_asset_files(temp_dir.path(), &runtime_assets, &message_metadata)?;
    let code_files = runtime_code_files(
        &runtime_assets,
        runtime_output.default_locale(),
        runtime_output.supported_locales(),
        &namespaces,
        &message_metadata,
    )?;
    let code_out_dir = options
        .out_dir_override()
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_config_relative_path(options.config_path(), &target.out_dir));
    let asset_out_dir = target
        .asset_dir
        .as_deref()
        .map(|asset_dir| resolve_config_relative_path(options.config_path(), asset_dir))
        .unwrap_or_else(|| code_out_dir.clone());
    let manifest_input = WebArtifactManifestInput {
        target: target.name.clone(),
        mode: target.mode,
        framework: target.framework,
        default_locale: runtime_output.default_locale().to_owned(),
        supported_locales: runtime_output.supported_locales().to_vec(),
        namespaces: namespaces.clone(),
    };

    if asset_out_dir == code_out_dir {
        let mut files = asset_files;
        files.extend(code_files);
        let asset_output = write_web_artifacts(
            &WebArtifactWriteOptions::new(asset_out_dir)
                .with_stale_policy(target.stale_policy.unwrap_or(WebStalePolicy::Fail))
                .with_check(options.check()),
            &manifest_input,
            files,
        )?;
        return Ok(RuntimeWebBuildOutput {
            asset_output,
            code_output: None,
            runtime_assets,
            default_locale: runtime_output.default_locale().to_owned(),
            supported_locales: runtime_output.supported_locales().to_vec(),
            namespaces,
            message_metadata,
        });
    }

    let asset_output = write_web_artifacts(
        &WebArtifactWriteOptions::new(asset_out_dir)
            .with_stale_policy(target.stale_policy.unwrap_or(WebStalePolicy::Fail))
            .with_check(options.check()),
        &manifest_input,
        asset_files,
    )?;
    let code_output = write_web_artifacts(
        &WebArtifactWriteOptions::new(code_out_dir)
            .with_stale_policy(target.stale_policy.unwrap_or(WebStalePolicy::Fail))
            .with_check(options.check()),
        &manifest_input,
        code_files,
    )?;

    Ok(RuntimeWebBuildOutput {
        asset_output,
        code_output: Some(code_output),
        runtime_assets,
        default_locale: runtime_output.default_locale().to_owned(),
        supported_locales: runtime_output.supported_locales().to_vec(),
        namespaces,
        message_metadata,
    })
}

fn runtime_assets_from_manifest(
    target: &str,
    manifest_path: &Path,
) -> Result<RuntimeWebAssets, RuntimeWebBuildError> {
    let manifest_bytes = fs::read(manifest_path)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    let packs = manifest
        .mf2_packs
        .iter()
        .map(|(locale, entry)| RuntimeWebPackAsset {
            locale: locale.clone(),
            url: entry.url.clone(),
            kind: entry.kind.clone(),
            hash: entry.hash.clone(),
            size: entry.size,
            parent: entry.parent.clone(),
        })
        .collect::<Vec<_>>();

    Ok(RuntimeWebAssets {
        schema: 1,
        target: target.to_owned(),
        mode: "runtime".to_string(),
        default_locale: manifest.default_locale,
        supported_locales: manifest.supported_locales,
        manifest: MANIFEST_FILE.to_string(),
        platform_bundle: PLATFORM_BUNDLE_FILE.to_string(),
        id_map: ID_MAP_JSON_FILE.to_string(),
        id_map_hash: ID_MAP_HASH_FILE.to_string(),
        metadata: "metadata.json".to_string(),
        packs,
    })
}

fn runtime_asset_files(
    runtime_dir: &Path,
    runtime_assets: &RuntimeWebAssets,
    message_metadata: &[WebMessageMetadata],
) -> Result<Vec<WebArtifactFile>, RuntimeWebBuildError> {
    let mut files = vec![
        read_artifact_file(runtime_dir, MANIFEST_FILE)?,
        read_artifact_file(runtime_dir, PLATFORM_BUNDLE_FILE)?,
        read_artifact_file(runtime_dir, ID_MAP_JSON_FILE)?,
        read_artifact_file(runtime_dir, ID_MAP_HASH_FILE)?,
        WebArtifactFile::new(RUNTIME_ASSETS_FILE, json_bytes(runtime_assets)?)?,
        WebArtifactFile::new("metadata.json", json_bytes(message_metadata)?)?,
    ];

    for pack in &runtime_assets.packs {
        files.push(read_artifact_file(runtime_dir, &pack.url)?);
    }

    Ok(files)
}

fn runtime_code_files(
    runtime_assets: &RuntimeWebAssets,
    default_locale: &str,
    supported_locales: &[String],
    namespaces: &[String],
    message_metadata: &[WebMessageMetadata],
) -> Result<Vec<WebArtifactFile>, RuntimeWebBuildError> {
    Ok(vec![
        WebArtifactFile::new(
            "types.ts",
            render_runtime_types(
                default_locale,
                supported_locales,
                namespaces,
                message_metadata,
            )
            .into_bytes(),
        )?,
        WebArtifactFile::new(
            "loaders.ts",
            render_runtime_loaders(runtime_assets)?.into_bytes(),
        )?,
        WebArtifactFile::new(
            "index.ts",
            b"export * from \"./types\";\nexport * from \"./loaders\";\n".to_vec(),
        )?,
    ])
}

fn read_artifact_file(
    runtime_dir: &Path,
    relative_path: &str,
) -> Result<WebArtifactFile, RuntimeWebBuildError> {
    Ok(WebArtifactFile::new(
        relative_path,
        fs::read(runtime_dir.join(relative_path))?,
    )?)
}

fn render_runtime_types(
    default_locale: &str,
    supported_locales: &[String],
    namespaces: &[String],
    message_metadata: &[WebMessageMetadata],
) -> String {
    format!(
        "{}export type Mf2RuntimeArg = string | number | boolean | {{ readonly type: \"datetime\"; readonly unixMilliseconds: number }} | {{ readonly type: \"datetime\"; readonly unixSeconds: number }} | {{ readonly type: \"currency\"; readonly value: number; readonly code: string }};\nexport type Mf2RuntimeArgs = Record<string, Mf2RuntimeArg>;\nexport type RuntimeWebPackAsset = {{ readonly locale: Locale; readonly url: string; readonly kind: string; readonly hash: string; readonly size: number; readonly parent?: Locale }};\nexport type RuntimeWebAssets = {{ readonly schema: number; readonly target: string; readonly mode: \"runtime\"; readonly defaultLocale: Locale; readonly supportedLocales: readonly Locale[]; readonly manifest: string; readonly platformBundle: string; readonly idMap: string; readonly idMapHash: string; readonly metadata: string; readonly packs: readonly RuntimeWebPackAsset[] }};\nexport type Mf2RuntimeParts = {{ readonly manifest: unknown; readonly idMap: unknown; readonly packs: Record<string, Uint8Array | ArrayBuffer> }};\nexport type Mf2RuntimeInstance = {{ defaultLocale(): string; supportedLocales(): string[]; negotiateLocale(requested?: string | readonly string[]): string; hasMessage(key: string): boolean; format(locale: string, key: string, args?: unknown): string }};\nexport type Mf2RuntimeClass = {{ fromParts(parts: Mf2RuntimeParts): Mf2RuntimeInstance }};\nexport type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;\nexport type RuntimeLoaderOptions = {{ readonly baseUrl?: string; readonly fetch?: FetchLike; readonly cacheKey?: string; readonly cache?: boolean }};\nexport type RuntimeFormatOptions = {{ readonly locales?: string | readonly string[] }};\n",
        render_web_typescript_metadata_types(
            default_locale,
            supported_locales,
            namespaces,
            message_metadata
        )
    )
}

fn render_runtime_loaders(
    runtime_assets: &RuntimeWebAssets,
) -> Result<String, RuntimeWebBuildError> {
    let assets_json = serde_json::to_string_pretty(runtime_assets)?;
    let mut output = String::new();
    output.push_str(
        "import type { ArgsFor, FetchLike, Locale, MessageId, Mf2RuntimeClass, Mf2RuntimeInstance, Mf2RuntimeParts, RuntimeFormatOptions, RuntimeLoaderOptions, RuntimeWebAssets, RuntimeWebPackAsset } from \"./types\";\n\n",
    );
    output.push_str("export const RUNTIME_ASSETS = ");
    output.push_str(&assets_json);
    output.push_str(" as RuntimeWebAssets;\n\n");
    output.push_str(
        r#"const runtimeCache = new Map<string, Promise<Mf2RuntimeInstance>>();
const jsonCache = new Map<string, Promise<unknown>>();
const packCache = new Map<string, Promise<Uint8Array>>();

export function resolveMf2AssetUrl(baseUrl: string, path: string): string {
  const cleanBase = baseUrl.replace(/\/+$/, "");
  const cleanPath = path.replace(/^\/+/, "");
  return cleanBase.length === 0 ? cleanPath : `${cleanBase}/${cleanPath}`;
}

export function runtimeManifestUrl(baseUrl = ""): string {
  return resolveMf2AssetUrl(baseUrl, RUNTIME_ASSETS.manifest);
}

export function runtimeIdMapUrl(baseUrl = ""): string {
  return resolveMf2AssetUrl(baseUrl, RUNTIME_ASSETS.idMap);
}

export function packAsset(locale: Locale): RuntimeWebPackAsset {
  const pack = RUNTIME_ASSETS.packs.find((entry) => entry.locale === locale);
  if (!pack) {
    throw new Error(`missing MF2 runtime pack for ${locale}`);
  }
  return pack;
}

export function packUrl(locale: Locale, baseUrl = ""): string {
  return resolveMf2AssetUrl(baseUrl, packAsset(locale).url);
}

export async function loadLocale(locale: Locale, options: RuntimeLoaderOptions = {}): Promise<Uint8Array> {
  const url = packUrl(locale, options.baseUrl ?? "");
  const cacheKey = `${options.cacheKey ?? options.baseUrl ?? ""}:pack:${locale}:${url}`;
  if (options.cache !== false) {
    const cached = packCache.get(cacheKey);
    if (cached) {
      return cached;
    }
  }
  const promise = fetchBytes(url, options);
  if (options.cache !== false) {
    packCache.set(cacheKey, promise);
  }
  return promise;
}

export async function loadRuntimeParts(options: RuntimeLoaderOptions = {}): Promise<Mf2RuntimeParts> {
  const baseUrl = options.baseUrl ?? "";
  const manifest = await fetchJsonCached(runtimeManifestUrl(baseUrl), options, "manifest");
  const idMap = await fetchJsonCached(runtimeIdMapUrl(baseUrl), options, "id-map");
  const packs: Record<string, Uint8Array> = {};
  await Promise.all(RUNTIME_ASSETS.supportedLocales.map(async (locale) => {
    packs[locale] = await loadLocale(locale, options);
  }));
  return { manifest, idMap, packs };
}

export async function createMf2Runtime(runtimeClass: Mf2RuntimeClass, options: RuntimeLoaderOptions = {}): Promise<Mf2RuntimeInstance> {
  const cacheKey = `${options.cacheKey ?? options.baseUrl ?? ""}:runtime`;
  if (options.cache !== false) {
    const cached = runtimeCache.get(cacheKey);
    if (cached) {
      return cached;
    }
  }
  const promise = loadRuntimeParts(options).then((parts) => runtimeClass.fromParts(parts));
  if (options.cache !== false) {
    runtimeCache.set(cacheKey, promise);
  }
  return promise;
}

export function format<TMessageId extends MessageId>(
  runtime: Mf2RuntimeInstance,
  messageId: TMessageId,
  args: ArgsFor<TMessageId>,
  options: RuntimeFormatOptions = {},
): string {
  if (!runtime.hasMessage(messageId)) {
    throw new Error(`missing MF2 message ${messageId}`);
  }
  const locale = runtime.negotiateLocale(options.locales ?? []);
  return runtime.format(locale, messageId, args ?? {});
}

async function fetchJsonCached(url: string, options: RuntimeLoaderOptions, label: string): Promise<unknown> {
  const cacheKey = `${options.cacheKey ?? options.baseUrl ?? ""}:json:${label}:${url}`;
  if (options.cache !== false) {
    const cached = jsonCache.get(cacheKey);
    if (cached) {
      return cached;
    }
  }
  const promise = fetchJson(url, options);
  if (options.cache !== false) {
    jsonCache.set(cacheKey, promise);
  }
  return promise;
}

async function fetchJson(url: string, options: RuntimeLoaderOptions): Promise<unknown> {
  const response = await fetchWith(options)(url);
  assertOk(response, url);
  return response.json();
}

async function fetchBytes(url: string, options: RuntimeLoaderOptions): Promise<Uint8Array> {
  const response = await fetchWith(options)(url);
  assertOk(response, url);
  return new Uint8Array(await response.arrayBuffer());
}

function fetchWith(options: RuntimeLoaderOptions): FetchLike {
  return options.fetch ?? globalThis.fetch.bind(globalThis);
}

function assertOk(response: Response, url: string): void {
  if (!response.ok) {
    throw new Error(`failed to load MF2 runtime asset ${url}: ${response.status}`);
  }
}
"#,
    );
    Ok(output)
}

fn json_bytes<T>(value: &T) -> Result<Vec<u8>, serde_json::Error>
where
    T: Serialize + ?Sized,
{
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn namespaces_from_metadata(message_metadata: &[WebMessageMetadata]) -> Vec<String> {
    message_metadata
        .iter()
        .map(|message| message.namespace.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct RuntimeTempDir {
    path: PathBuf,
}

impl RuntimeTempDir {
    fn new(target: &str) -> Result<Self, std::io::Error> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mf2_i18n_runtime_web_{}_{}_{}",
            sanitize_temp_name(target),
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RuntimeTempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).ok();
    }
}

fn sanitize_temp_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{RuntimeWebBuildOptions, build_runtime_web_target};
    use crate::web::WEB_ARTIFACT_MANIFEST_FILE;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_web_runtime_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_catalog(path: &Path, messages: &[(&str, &str)]) {
        let contents = messages
            .iter()
            .map(|(key, value)| format!("  {key:?}: {value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        fs::write(path, format!("{{\n{contents}\n}}\n")).expect("write");
    }

    fn write_fixture(root: &Path) -> PathBuf {
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let spanish_dir = locales_root.join("es");
        fs::create_dir_all(&english_dir).expect("en");
        fs::create_dir_all(&spanish_dir).expect("es");
        write_catalog(
            &english_dir.join("common.json"),
            &[
                ("home.title", "Hi"),
                ("cart.count", "{ $count :number } items"),
            ],
        );
        write_catalog(
            &spanish_dir.join("common.json"),
            &[
                ("home.title", "Hola"),
                ("cart.count", "{ $count :number } articulos"),
            ],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");
        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"runtime\"\nmode = \"runtime\"\nout_dir = \"generated/runtime\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"common\"]\nstale_policy = \"fail\"\n",
        )
        .expect("config");
        config_path
    }

    #[test]
    fn builds_runtime_web_target_assets_and_loaders() {
        let root = temp_dir("target");
        let config_path = write_fixture(&root);

        let output =
            build_runtime_web_target(&RuntimeWebBuildOptions::new(&config_path, "runtime"))
                .expect("runtime web build");
        let out_dir = root.join("generated/runtime");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "es".to_string()]
        );
        assert_eq!(output.namespaces(), &["common".to_string()]);
        assert_eq!(output.message_metadata().len(), 2);
        assert!(output.code_output().is_none());
        assert!(out_dir.join(WEB_ARTIFACT_MANIFEST_FILE).exists());
        assert!(out_dir.join("manifest.json").exists());
        assert!(out_dir.join("platform-bundle.json").exists());
        assert!(out_dir.join("id-map.json").exists());
        assert!(out_dir.join("id-map.sha256").exists());
        assert!(out_dir.join("runtime-assets.json").exists());
        assert!(out_dir.join("metadata.json").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());
        assert!(out_dir.join("packs/es.mf2pack").exists());

        let runtime_assets =
            fs::read_to_string(out_dir.join("runtime-assets.json")).expect("assets");
        assert!(runtime_assets.contains("\"target\": \"runtime\""));

        let types = fs::read_to_string(out_dir.join("types.ts")).expect("types");
        assert!(types.contains("\"common.cart.count\""));
        assert!(types.contains("readonly \"count\": number"));
        assert!(types.contains("Mf2RuntimeClass"));
        assert!(types.contains("Mf2RuntimeParts"));

        let loaders = fs::read_to_string(out_dir.join("loaders.ts")).expect("loaders");
        assert!(loaders.contains("RUNTIME_ASSETS"));
        assert!(loaders.contains("createMf2Runtime"));
        assert!(loaders.contains("loadRuntimeParts"));
        assert!(loaders.contains("loadLocale"));
        assert!(loaders.contains("runtimeCache"));
        assert!(loaders.contains("packCache"));
        assert!(loaders.contains("packUrl"));
        assert!(loaders.contains("export function format"));

        fs::remove_dir_all(&root).ok();
    }
}
