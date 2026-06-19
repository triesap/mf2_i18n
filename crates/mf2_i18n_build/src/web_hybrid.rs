use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::manifest::Manifest;
use crate::project::{ProjectError, ProjectLayout, resolve_config_relative_path};
use crate::project_catalogs::{ProjectCatalog, ProjectCatalogError, ProjectCatalogMessage};
use crate::project_pipeline::{
    NativeModuleBuildError, ProjectRuntimeBuildOptions, build_project_runtime_artifacts,
};
use crate::web::{
    WebArtifactError, WebArtifactFile, WebArtifactManifestInput, WebArtifactWriteOptions,
    WebArtifactWriteOutput, WebDtoTypeArtifactError, append_web_dto_artifacts, write_web_artifacts,
};
use crate::web_json::{WebJsonExportError, insert_nested_message, plain_text_message};
use crate::web_metadata::{
    HybridMessagePlan, WebMessageMetadata, WebMessageMetadataError, classify_hybrid_messages,
    collect_web_message_metadata, render_web_typescript_metadata_types,
};
use crate::web_runtime::{RuntimeWebAssets, RuntimeWebPackAsset};
use crate::{WebStalePolicy, WebTargetConfigError, WebTargetMode};

const DEFAULT_HYBRID_GENERATED_AT: &str = "1970-01-01T00:00:00Z";
const HYBRID_ASSETS_FILE: &str = "hybrid-assets.json";
const RUNTIME_ASSETS_FILE: &str = "runtime-assets.json";
const MANIFEST_FILE: &str = "manifest.json";
const PLATFORM_BUNDLE_FILE: &str = "platform-bundle.json";
const ID_MAP_JSON_FILE: &str = "id-map.json";
const ID_MAP_HASH_FILE: &str = "id-map.sha256";
const MESSAGES_DIR: &str = "messages";

#[derive(Debug, Clone)]
pub struct HybridWebBuildOptions {
    config_path: PathBuf,
    target_name: String,
    out_dir_override: Option<PathBuf>,
    check: bool,
    dto_types: bool,
}

impl HybridWebBuildOptions {
    pub fn new(config_path: impl Into<PathBuf>, target_name: impl Into<String>) -> Self {
        Self {
            config_path: config_path.into(),
            target_name: target_name.into(),
            out_dir_override: None,
            check: false,
            dto_types: false,
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

    pub fn with_dto_types(mut self, dto_types: bool) -> Self {
        self.dto_types = dto_types;
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

    pub fn dto_types(&self) -> bool {
        self.dto_types
    }
}

#[derive(Debug, Clone)]
pub struct HybridWebBuildOutput {
    artifact_output: WebArtifactWriteOutput,
    hybrid_assets: HybridWebAssets,
    runtime_assets: RuntimeWebAssets,
    message_plan: HybridMessagePlan,
    default_locale: String,
    supported_locales: Vec<String>,
    namespaces: Vec<String>,
    message_metadata: Vec<WebMessageMetadata>,
}

impl HybridWebBuildOutput {
    pub fn artifact_output(&self) -> &WebArtifactWriteOutput {
        &self.artifact_output
    }

    pub fn hybrid_assets(&self) -> &HybridWebAssets {
        &self.hybrid_assets
    }

    pub fn runtime_assets(&self) -> &RuntimeWebAssets {
        &self.runtime_assets
    }

    pub fn message_plan(&self) -> &HybridMessagePlan {
        &self.message_plan
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
pub struct HybridWebAssets {
    pub schema: u32,
    pub target: String,
    pub mode: String,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub static_messages_root: String,
    pub runtime_assets: String,
    pub metadata: String,
    pub static_message_ids: Vec<String>,
    pub runtime_message_ids: Vec<String>,
}

#[derive(Debug, Error)]
pub enum HybridWebBuildError {
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
    WebJson(#[from] WebJsonExportError),
    #[error(transparent)]
    Artifacts(#[from] WebArtifactError),
    #[error(transparent)]
    DtoTypes(#[from] WebDtoTypeArtifactError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("missing web target {0}")]
    MissingTarget(String),
    #[error("web target {target} must use hybrid mode")]
    UnsupportedTargetMode { target: String },
}

pub fn build_hybrid_web_target(
    options: &HybridWebBuildOptions,
) -> Result<HybridWebBuildOutput, HybridWebBuildError> {
    let project = ProjectLayout::load_or_default(options.config_path())?;
    project.config().validate_web_targets()?;
    let target = project
        .config()
        .web
        .target(options.target_name())
        .ok_or_else(|| HybridWebBuildError::MissingTarget(options.target_name().to_owned()))?
        .clone();
    if target.mode != WebTargetMode::Hybrid {
        return Err(HybridWebBuildError::UnsupportedTargetMode {
            target: target.name.clone(),
        });
    }

    let loaded_catalogs = crate::project_catalogs::load_project_catalogs(&project)?;
    let default_locale = project.config().default_locale.clone();
    let default_catalog = loaded_catalogs
        .catalog(&default_locale)
        .ok_or_else(|| ProjectCatalogError::MissingDefaultLocale(default_locale.clone()))?;
    let namespace_filter = (!target.namespaces.is_empty()).then_some(target.namespaces.as_slice());
    let message_metadata =
        collect_web_message_metadata(&default_locale, default_catalog, namespace_filter)?;
    let message_plan =
        classify_hybrid_messages(&default_locale, default_catalog, namespace_filter)?;
    let namespaces = namespaces_from_metadata(&message_metadata);
    let supported_locales = filtered_supported_locales(
        loaded_catalogs.catalogs().keys().cloned().collect(),
        &target.locales,
    );
    let static_message_ids = message_plan
        .static_messages
        .iter()
        .map(|route| route.id.clone())
        .collect::<BTreeSet<_>>();

    let temp_dir = HybridTempDir::new(&target.name)?;
    let runtime_output = build_project_runtime_artifacts(
        &ProjectRuntimeBuildOptions::new(
            options.config_path(),
            temp_dir.path(),
            target.release_id.as_deref().unwrap_or(&target.name),
            target
                .generated_at
                .as_deref()
                .unwrap_or(DEFAULT_HYBRID_GENERATED_AT),
        )
        .with_locales(target.locales.clone())
        .with_namespaces(target.namespaces.clone()),
    )?;
    let runtime_assets =
        runtime_assets_from_manifest(&target.name, runtime_output.manifest_path())?;
    let hybrid_assets = HybridWebAssets {
        schema: 1,
        target: target.name.clone(),
        mode: "hybrid".to_string(),
        default_locale: runtime_output.default_locale().to_owned(),
        supported_locales: runtime_output.supported_locales().to_vec(),
        static_messages_root: MESSAGES_DIR.to_string(),
        runtime_assets: RUNTIME_ASSETS_FILE.to_string(),
        metadata: "metadata.json".to_string(),
        static_message_ids: message_plan
            .static_messages
            .iter()
            .map(|route| route.id.clone())
            .collect(),
        runtime_message_ids: message_plan
            .runtime_messages
            .iter()
            .map(|route| route.id.clone())
            .collect(),
    };
    let static_namespaces = message_plan
        .static_messages
        .iter()
        .map(|route| route.namespace.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let mut artifact_files = runtime_artifact_files(temp_dir.path(), &runtime_assets)?;
    artifact_files.extend(static_message_files(
        loaded_catalogs.catalogs(),
        &target.locales,
        &target.namespaces,
        &static_message_ids,
    )?);
    artifact_files.push(WebArtifactFile::new(
        HYBRID_ASSETS_FILE,
        json_bytes(&hybrid_assets)?,
    )?);
    artifact_files.push(WebArtifactFile::new(
        RUNTIME_ASSETS_FILE,
        json_bytes(&runtime_assets)?,
    )?);
    artifact_files.push(WebArtifactFile::new(
        "metadata.json",
        json_bytes(&message_metadata)?,
    )?);
    artifact_files.push(WebArtifactFile::new(
        "types.ts",
        render_hybrid_types(
            runtime_output.default_locale(),
            runtime_output.supported_locales(),
            &namespaces,
            &message_metadata,
        )
        .into_bytes(),
    )?);
    artifact_files.push(WebArtifactFile::new(
        "loaders.ts",
        render_hybrid_loaders(
            &hybrid_assets,
            &runtime_assets,
            runtime_output.supported_locales(),
            &static_namespaces,
        )?
        .into_bytes(),
    )?);
    artifact_files.push(WebArtifactFile::new(
        "index.ts",
        b"export * from \"./types\";\nexport * from \"./loaders\";\n".to_vec(),
    )?);
    if options.dto_types() {
        append_web_dto_artifacts(&mut artifact_files)?;
    }

    let out_dir = options
        .out_dir_override()
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_config_relative_path(options.config_path(), &target.out_dir));
    let artifact_output = write_web_artifacts(
        &WebArtifactWriteOptions::new(out_dir)
            .with_stale_policy(target.stale_policy.unwrap_or(WebStalePolicy::Fail))
            .with_check(options.check()),
        &WebArtifactManifestInput {
            target: target.name.clone(),
            mode: target.mode,
            framework: target.framework,
            default_locale: runtime_output.default_locale().to_owned(),
            supported_locales: runtime_output.supported_locales().to_vec(),
            namespaces: namespaces.clone(),
        },
        artifact_files,
    )?;

    Ok(HybridWebBuildOutput {
        artifact_output,
        hybrid_assets,
        runtime_assets,
        message_plan,
        default_locale: runtime_output.default_locale().to_owned(),
        supported_locales,
        namespaces,
        message_metadata,
    })
}

fn runtime_assets_from_manifest(
    target: &str,
    manifest_path: &Path,
) -> Result<RuntimeWebAssets, HybridWebBuildError> {
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
        mode: "hybrid".to_string(),
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

fn runtime_artifact_files(
    runtime_dir: &Path,
    runtime_assets: &RuntimeWebAssets,
) -> Result<Vec<WebArtifactFile>, HybridWebBuildError> {
    let mut files = vec![
        read_artifact_file(runtime_dir, MANIFEST_FILE)?,
        read_artifact_file(runtime_dir, PLATFORM_BUNDLE_FILE)?,
        read_artifact_file(runtime_dir, ID_MAP_JSON_FILE)?,
        read_artifact_file(runtime_dir, ID_MAP_HASH_FILE)?,
    ];

    for pack in &runtime_assets.packs {
        files.push(read_artifact_file(runtime_dir, &pack.url)?);
    }

    Ok(files)
}

fn read_artifact_file(
    runtime_dir: &Path,
    relative_path: &str,
) -> Result<WebArtifactFile, HybridWebBuildError> {
    Ok(WebArtifactFile::new(
        relative_path,
        fs::read(runtime_dir.join(relative_path))?,
    )?)
}

fn static_message_files(
    catalogs: &BTreeMap<String, ProjectCatalog>,
    locale_filter: &[String],
    namespace_filter: &[String],
    static_message_ids: &BTreeSet<String>,
) -> Result<Vec<WebArtifactFile>, HybridWebBuildError> {
    let mut namespace_maps = BTreeMap::<(String, String), Map<String, Value>>::new();

    for (locale, catalog) in catalogs {
        if !locale_filter.is_empty() && !locale_filter.contains(locale) {
            continue;
        }
        for message in catalog.values() {
            if !namespace_filter.is_empty() && !namespace_filter.contains(&message.namespace) {
                continue;
            }
            if !static_message_ids.contains(&message.qualified_key) {
                continue;
            }
            insert_static_message(&mut namespace_maps, locale, message)?;
        }
    }

    namespace_maps
        .into_iter()
        .map(|((locale, namespace), map)| {
            let mut bytes = serde_json::to_vec_pretty(&Value::Object(map))?;
            bytes.push(b'\n');
            Ok(WebArtifactFile::new(
                Path::new(MESSAGES_DIR)
                    .join(locale)
                    .join(format!("{namespace}.json")),
                bytes,
            )?)
        })
        .collect()
}

fn insert_static_message(
    namespace_maps: &mut BTreeMap<(String, String), Map<String, Value>>,
    locale: &str,
    message: &ProjectCatalogMessage,
) -> Result<(), HybridWebBuildError> {
    let text = plain_text_message(locale, message)?;
    let namespace_map = namespace_maps
        .entry((locale.to_owned(), message.namespace.clone()))
        .or_default();
    insert_nested_message(namespace_map, locale, message, text)?;
    Ok(())
}

fn render_hybrid_types(
    default_locale: &str,
    supported_locales: &[String],
    namespaces: &[String],
    message_metadata: &[WebMessageMetadata],
) -> String {
    format!(
        "{}export type StaticMessages = Record<string, unknown>;\nexport type Mf2RuntimeArg = string | number | boolean | {{ readonly type: \"datetime\"; readonly unixMilliseconds: number }} | {{ readonly type: \"datetime\"; readonly unixSeconds: number }} | {{ readonly type: \"currency\"; readonly value: number; readonly code: string }};\nexport type Mf2RuntimeArgs = Record<string, Mf2RuntimeArg>;\nexport type RuntimeWebPackAsset = {{ readonly locale: Locale; readonly url: string; readonly kind: string; readonly hash: string; readonly size: number; readonly parent?: Locale }};\nexport type RuntimeWebAssets = {{ readonly schema: number; readonly target: string; readonly mode: \"runtime\" | \"hybrid\"; readonly defaultLocale: Locale; readonly supportedLocales: readonly Locale[]; readonly manifest: string; readonly platformBundle: string; readonly idMap: string; readonly idMapHash: string; readonly metadata: string; readonly packs: readonly RuntimeWebPackAsset[] }};\nexport type HybridWebAssets = {{ readonly schema: number; readonly target: string; readonly mode: \"hybrid\"; readonly defaultLocale: Locale; readonly supportedLocales: readonly Locale[]; readonly staticMessagesRoot: string; readonly runtimeAssets: string; readonly metadata: string; readonly staticMessageIds: readonly MessageId[]; readonly runtimeMessageIds: readonly MessageId[] }};\nexport type Mf2RuntimeParts = {{ readonly manifest: unknown; readonly idMap: unknown; readonly packs: Record<string, Uint8Array | ArrayBuffer> }};\nexport type Mf2RuntimeInstance = {{ defaultLocale(): string; supportedLocales(): string[]; negotiateLocale(requested?: string | readonly string[]): string; hasMessage(key: string): boolean; format(locale: string, key: string, args?: unknown): string }};\nexport type Mf2RuntimeClass = {{ fromParts(parts: Mf2RuntimeParts): Mf2RuntimeInstance }};\nexport type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;\nexport type HybridLoaderOptions = {{ readonly baseUrl?: string; readonly fetch?: FetchLike; readonly cacheKey?: string; readonly cache?: boolean }};\nexport type HybridFormatOptions = HybridLoaderOptions & {{ readonly locales?: string | readonly string[] }};\n",
        render_web_typescript_metadata_types(
            default_locale,
            supported_locales,
            namespaces,
            message_metadata
        )
    )
}

fn render_hybrid_loaders(
    hybrid_assets: &HybridWebAssets,
    runtime_assets: &RuntimeWebAssets,
    supported_locales: &[String],
    static_namespaces: &[String],
) -> Result<String, HybridWebBuildError> {
    let hybrid_assets_json = serde_json::to_string_pretty(hybrid_assets)?;
    let runtime_assets_json = serde_json::to_string_pretty(runtime_assets)?;
    let mut loader_entries = Vec::new();
    for locale in supported_locales {
        for namespace in static_namespaces {
            let import_path = format!("./{MESSAGES_DIR}/{locale}/{namespace}.json");
            loader_entries.push(format!(
                "  {{\n    locale: {},\n    namespace: {},\n    loader: async () => (await import({})).default as StaticMessages,\n  }}",
                json_string(locale),
                json_string(namespace),
                json_string(&import_path)
            ));
        }
    }
    let static_loaders = if loader_entries.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", loader_entries.join(",\n"))
    };
    let mut output = String::new();
    output.push_str(
        "import { MESSAGE_NAMESPACES } from \"./types\";\nimport type { ArgsFor, FetchLike, HybridFormatOptions, HybridLoaderOptions, HybridWebAssets, Locale, MessageId, Mf2RuntimeClass, Mf2RuntimeInstance, Mf2RuntimeParts, Namespace, RuntimeWebAssets, RuntimeWebPackAsset, StaticMessages } from \"./types\";\n\n",
    );
    output.push_str("export const HYBRID_ASSETS = ");
    output.push_str(&hybrid_assets_json);
    output.push_str(" as HybridWebAssets;\n\n");
    output.push_str("export const RUNTIME_ASSETS = ");
    output.push_str(&runtime_assets_json);
    output.push_str(" as RuntimeWebAssets;\n\n");
    output.push_str("export const STATIC_MESSAGE_LOADERS = [");
    output.push_str(&static_loaders);
    output.push_str("] as const;\n\n");
    output.push_str(
        r#"const runtimeCache = new Map<string, Promise<Mf2RuntimeInstance>>();
const jsonCache = new Map<string, Promise<unknown>>();
const packCache = new Map<string, Promise<Uint8Array>>();

export function isStaticMessage(messageId: MessageId): boolean {
  return HYBRID_ASSETS.staticMessageIds.includes(messageId);
}

export function isRuntimeMessage(messageId: MessageId): boolean {
  return HYBRID_ASSETS.runtimeMessageIds.includes(messageId);
}

export function negotiateLocale(requested: string | readonly string[] = []): Locale {
  const requestedLocales = typeof requested === "string" ? [requested] : requested;
  for (const locale of requestedLocales) {
    const match = lookupLocale(locale);
    if (match) {
      return match;
    }
  }
  return HYBRID_ASSETS.defaultLocale;
}

export async function loadStaticMessages(locale: Locale, namespace: Namespace, options: HybridLoaderOptions = {}): Promise<StaticMessages> {
  const entry = STATIC_MESSAGE_LOADERS.find((loader) => loader.locale === locale && loader.namespace === namespace);
  if (!entry) {
    throw new Error(`missing MF2 static messages for ${locale}/${namespace}`);
  }
  const cacheKey = `${options.cacheKey ?? ""}:static:${locale}:${namespace}`;
  if (options.cache !== false) {
    const cached = jsonCache.get(cacheKey);
    if (cached) {
      return cached as Promise<StaticMessages>;
    }
  }
  const promise = entry.loader();
  if (options.cache !== false) {
    jsonCache.set(cacheKey, promise);
  }
  return promise;
}

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

export async function loadLocale(locale: Locale, options: HybridLoaderOptions = {}): Promise<Uint8Array> {
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

export async function loadRuntimeParts(options: HybridLoaderOptions = {}): Promise<Mf2RuntimeParts> {
  const baseUrl = options.baseUrl ?? "";
  const manifest = await fetchJsonCached(runtimeManifestUrl(baseUrl), options, "manifest");
  const idMap = await fetchJsonCached(runtimeIdMapUrl(baseUrl), options, "id-map");
  const packs: Record<string, Uint8Array> = {};
  await Promise.all(RUNTIME_ASSETS.supportedLocales.map(async (locale) => {
    packs[locale] = await loadLocale(locale, options);
  }));
  return { manifest, idMap, packs };
}

export async function createMf2Runtime(runtimeClass: Mf2RuntimeClass, options: HybridLoaderOptions = {}): Promise<Mf2RuntimeInstance> {
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

export async function format<TMessageId extends MessageId>(
  runtimeClass: Mf2RuntimeClass,
  messageId: TMessageId,
  args: ArgsFor<TMessageId>,
  options: HybridFormatOptions = {},
): Promise<string> {
  if (isStaticMessage(messageId)) {
    const locale = negotiateLocale(options.locales ?? []);
    const namespace = namespaceForMessage(messageId);
    const messages = await loadStaticMessages(locale, namespace, options);
    const value = readStaticMessage(messages, namespace, messageId);
    if (value !== undefined) {
      return value;
    }
  }
  const runtime = await createMf2Runtime(runtimeClass, options);
  if (!runtime.hasMessage(messageId)) {
    throw new Error(`missing MF2 message ${messageId}`);
  }
  const locale = runtime.negotiateLocale(options.locales ?? []);
  return runtime.format(locale, messageId, args ?? {});
}

function namespaceForMessage(messageId: MessageId): Namespace {
  const namespace = [...MESSAGE_NAMESPACES].sort((left, right) => right.length - left.length).find((value) => messageId.startsWith(`${value}.`));
  if (!namespace) {
    throw new Error(`missing MF2 namespace for ${messageId}`);
  }
  return namespace;
}

function readStaticMessage(messages: StaticMessages, namespace: Namespace, messageId: MessageId): string | undefined {
  const relativePath = messageId.slice(namespace.length + 1);
  const value = relativePath.split(".").reduce<unknown>((current, segment) => {
    if (!current || typeof current !== "object" || !(segment in current)) {
      return undefined;
    }
    return (current as Record<string, unknown>)[segment];
  }, messages);
  return typeof value === "string" ? value : undefined;
}

function lookupLocale(locale: string): Locale | undefined {
  let candidate = locale.toLowerCase().replace(/_/g, "-");
  while (candidate.length > 0) {
    const match = HYBRID_ASSETS.supportedLocales.find((supported) => supported.toLowerCase() === candidate);
    if (match) {
      return match;
    }
    const next = candidate.lastIndexOf("-");
    if (next < 0) {
      return undefined;
    }
    candidate = candidate.slice(0, next);
  }
  return undefined;
}

async function fetchJsonCached(url: string, options: HybridLoaderOptions, label: string): Promise<unknown> {
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

async function fetchJson(url: string, options: HybridLoaderOptions): Promise<unknown> {
  const response = await fetchWith(options)(url);
  assertOk(response, url);
  return response.json();
}

async function fetchBytes(url: string, options: HybridLoaderOptions): Promise<Uint8Array> {
  const response = await fetchWith(options)(url);
  assertOk(response, url);
  return new Uint8Array(await response.arrayBuffer());
}

function fetchWith(options: HybridLoaderOptions): FetchLike {
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

fn filtered_supported_locales(
    mut catalog_locales: Vec<String>,
    target_locales: &[String],
) -> Vec<String> {
    if target_locales.is_empty() {
        catalog_locales.sort();
        catalog_locales
    } else {
        target_locales.to_vec()
    }
}

fn namespaces_from_metadata(message_metadata: &[WebMessageMetadata]) -> Vec<String> {
    message_metadata
        .iter()
        .map(|message| message.namespace.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn json_bytes<T>(value: &T) -> Result<Vec<u8>, serde_json::Error>
where
    T: Serialize + ?Sized,
{
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization should not fail")
}

struct HybridTempDir {
    path: PathBuf,
}

impl HybridTempDir {
    fn new(target: &str) -> Result<Self, std::io::Error> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mf2_i18n_hybrid_web_{}_{}_{}",
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

impl Drop for HybridTempDir {
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
    use super::{HybridWebBuildOptions, build_hybrid_web_target};
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
        path.push(format!("mf2_i18n_web_hybrid_{name}_{nanos}"));
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
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"hybrid\"\nmode = \"hybrid\"\nframework = \"vite\"\nout_dir = \"generated/hybrid\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"common\"]\nstale_policy = \"fail\"\n",
        )
        .expect("config");
        config_path
    }

    #[test]
    fn builds_hybrid_web_target_assets() {
        let root = temp_dir("target");
        let config_path = write_fixture(&root);

        let output = build_hybrid_web_target(&HybridWebBuildOptions::new(&config_path, "hybrid"))
            .expect("hybrid web build");
        let out_dir = root.join("generated/hybrid");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "es".to_string()]
        );
        assert_eq!(output.namespaces(), &["common".to_string()]);
        assert_eq!(output.message_metadata().len(), 2);
        assert_eq!(output.message_plan().static_messages.len(), 1);
        assert_eq!(output.message_plan().runtime_messages.len(), 1);
        assert_eq!(
            output.hybrid_assets().static_message_ids,
            vec!["common.home.title".to_string()]
        );
        assert_eq!(
            output.hybrid_assets().runtime_message_ids,
            vec!["common.cart.count".to_string()]
        );
        assert_eq!(output.runtime_assets().mode, "hybrid");
        assert!(out_dir.join(WEB_ARTIFACT_MANIFEST_FILE).exists());
        assert!(out_dir.join("hybrid-assets.json").exists());
        assert!(out_dir.join("runtime-assets.json").exists());
        assert!(out_dir.join("manifest.json").exists());
        assert!(out_dir.join("platform-bundle.json").exists());
        assert!(out_dir.join("id-map.json").exists());
        assert!(out_dir.join("id-map.sha256").exists());
        assert!(out_dir.join("metadata.json").exists());
        assert!(out_dir.join("types.ts").exists());
        assert!(out_dir.join("loaders.ts").exists());
        assert!(out_dir.join("index.ts").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());
        assert!(out_dir.join("packs/es.mf2pack").exists());
        assert!(out_dir.join("messages/en/common.json").exists());
        assert!(out_dir.join("messages/es/common.json").exists());

        let common_json =
            fs::read_to_string(out_dir.join("messages/en/common.json")).expect("common json");
        let common: serde_json::Value = serde_json::from_str(&common_json).expect("json");
        assert_eq!(common["home"]["title"], "Hi");
        assert!(common["cart"].is_null());

        let hybrid_assets = fs::read_to_string(out_dir.join("hybrid-assets.json")).expect("assets");
        assert!(hybrid_assets.contains("\"mode\": \"hybrid\""));
        assert!(hybrid_assets.contains("\"common.home.title\""));
        assert!(hybrid_assets.contains("\"common.cart.count\""));

        let types = fs::read_to_string(out_dir.join("types.ts")).expect("types");
        assert!(types.contains("HybridWebAssets"));
        assert!(types.contains("\"common.cart.count\""));
        assert!(types.contains("readonly \"count\": number"));

        let loaders = fs::read_to_string(out_dir.join("loaders.ts")).expect("loaders");
        assert!(loaders.contains("import { MESSAGE_NAMESPACES }"));
        assert!(loaders.contains("FetchLike"));
        assert!(loaders.contains("HYBRID_ASSETS"));
        assert!(loaders.contains("STATIC_MESSAGE_LOADERS"));
        assert!(loaders.contains("createMf2Runtime"));
        assert!(loaders.contains("loadRuntimeParts"));
        assert!(loaders.contains("isStaticMessage"));
        assert!(loaders.contains("export async function format"));

        fs::remove_dir_all(&root).ok();
    }
}
