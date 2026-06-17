# mf2_i18n Web JS Runtime And Generator RCLD

Status: active
Created: 2026-06-17
Owner: mf2_i18n

## Goal

Make `mf2_i18n` a world-class pure-Rust localization foundation for JS-based
websites and frameworks by adding deterministic web artifact generation,
typed JavaScript and TypeScript API surfaces, framework-oriented output modes,
browser WASM runtime ergonomics, and deploy-safe validation.

This work must keep the Rust crates pure Rust. Generated JS, TypeScript, JSON,
and framework files are allowed as build outputs, but the Rust crate family must
not depend on Node tooling, npm packages, or framework runtimes.

## Product Standard

The target web integration must let a website declare localization intent once
in `mf2_i18n.toml`, run a single CLI command, and receive a deterministic set
of generated artifacts suitable for the selected runtime and framework.

Downstream websites should not need custom scripts for:

- splitting locale and namespace assets
- deriving TypeScript message id unions
- typing message arguments
- loading MF2 packs and id maps
- caching locale data
- generating Docusaurus-native `code.json` and localized MDX files
- checking generated output drift
- detecting stale locale or namespace files

## Approved Decisions

1. The first implementation favors generated app-local JS and TypeScript files.
   Publishing npm packages is deferred until generated contracts stabilize.
2. First production modes are `plain-json`, `runtime`, and `docusaurus`.
   `hybrid` is deferred until the plain JSON and WASM runtime foundations are
   both implemented and proven.
3. Docusaurus support includes native `code.json`, navbar/sidebar translation
   artifacts, and localized docs MDX generation.
4. JS extraction is deferred to a later slice. The first implementation
   generates robust outputs from checked-in catalogs.
5. Browser unit formatting must not be advertised unless it is implemented.
   Until real unit formatting exists, generated TypeScript arg contracts must
   omit or reject `unit` for web targets.

## Current State

The crate family already has strong foundations:

- `mf2_i18n_core` is no-std-capable core MF2 execution logic.
- `mf2_i18n_build` compiles catalogs into deterministic runtime artifacts.
- `mf2_i18n_runtime` validates manifests, id maps, pack sizes, pack hashes,
  pack schemas, locales, and overlay parents.
- `mf2_i18n_wasm` exposes a low-level browser runtime constructor.
- `mf2_i18n_cli export-web-json` emits literal-only JSON plus a TypeScript
  loader manifest.

The gaps to close are product-surface gaps rather than fundamental runtime
gaps:

- web target configuration is not represented in `mf2_i18n.toml`
- `export-web-json` only supports literal messages
- TypeScript output lacks typed message ids and typed args
- WASM callers must manually assemble manifests, id maps, and packs
- Docusaurus consumers need app-local sync scripts
- generated output is not staged and atomically replaced
- stale generated files are not detected

## Target Architecture

### Rust Crate Integration Contract

The implementation must align the full `mf2_i18n` Rust crate family behind one
coherent expert API surface:

- `mf2_i18n_core` remains the pure MF2 execution and type foundation.
- `mf2_i18n_build` owns project config loading, catalog reading, validation,
  deterministic artifact generation, and web target generation APIs.
- `mf2_i18n_cli` exposes the operator-facing `build-web` command and keeps CLI
  behavior as a thin wrapper over the build APIs.
- `mf2_i18n_runtime` remains the validated runtime artifact loader and format
  executor for native and WASM consumers.
- `mf2_i18n_wasm` exposes browser-safe runtime bindings and stable JS error
  contracts.
- `mf2_i18n` re-exports the public build/runtime surfaces through intentional
  feature gates so most consumers can depend on the root crate.

No downstream website should need to import lower-level crates directly unless
it intentionally owns a lower-level integration boundary.

### Crate and Feature Surfaces

Add a pure-Rust web generation layer. The preferred shape is a new
`mf2_i18n_web` crate consumed by `mf2_i18n_build`, or a `web` module inside
`mf2_i18n_build` if a separate crate adds no real isolation during the first
slice.

The main `mf2_i18n` crate should expose the web build APIs behind a feature:

```toml
[dependencies]
mf2_i18n = { version = "0.1.0", features = ["web_build"] }
```

Expected feature posture:

- `web_build`: pure-Rust generators and web target config types
- `wasm`: browser WASM bindings remain in `mf2_i18n_wasm`
- `web_extract`: optional later parser support for JS/TS/JSX/TSX extraction

No feature may introduce Node, npm, framework, or non-Rust runtime dependency
requirements.

### Configuration Contract

Extend `mf2_i18n.toml` with web targets:

```toml
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "docs"
mode = "docusaurus"
framework = "docusaurus"
out_dir = "src/generated/mf2"
asset_dir = "static/mf2"
locales = ["en", "es"]
namespaces = ["home", "docs", "docusaurus"]
docs_source_dir = "docs"
docusaurus_i18n_dir = "i18n"
```

Required fields:

- `name`
- `mode`
- `out_dir`

Optional fields:

- `framework`
- `asset_dir`
- `locales`
- `namespaces`
- `docs_source_dir`
- `docusaurus_i18n_dir`
- `generated_at`
- `release_id`
- `stale_policy`

Modes and framework presets must validate their required fields at build time.

### CLI Contract

Add:

```sh
mf2_i18n_cli build-web --target <name> [--config <path>] [--out <dir>] [--check]
```

Behavior:

- reads `mf2_i18n.toml`
- resolves the named web target
- generates all artifacts for that target
- stages output into a temporary sibling directory
- validates generated artifacts
- atomically replaces the target output where possible
- supports `--check` to verify generated output is current without writing

Keep `export-web-json` during migration only if needed. Its behavior should be
treated as a subset of `build-web --mode plain-json`, not as the strategic API.

### Output Modes

#### `plain-json`

Literal-only mode. This preserves current safety rules: variables, formatters,
selects, and plurals fail the build.

Generated files:

- `manifest.json`
- `messages/<locale>/<namespace>.json`
- `types.ts`
- `loaders.ts`
- `index.ts`

#### `runtime`

Full MF2 runtime mode for JS and browser frameworks.

Generated files:

- `manifest.json`
- `id-map.json`
- `packs/<locale>.mf2pack`
- `runtime-assets.json`
- `types.ts`
- `loaders.ts`
- `runtime.ts`
- `index.ts`

The generated `runtime.ts` must provide a high-level loader around
`mf2_i18n_wasm`:

- lazy-load locale packs
- cache loaded packs
- verify manifest and pack metadata through the WASM/Rust runtime
- expose a typed `format` API
- expose locale negotiation helpers

#### `docusaurus`

Docusaurus-native output mode.

Generated files:

- Docusaurus `i18n/<locale>/code.json`
- Docusaurus theme translation files such as navbar translations
- Docusaurus docs plugin translation metadata
- localized docs MDX files under Docusaurus-native paths
- generated MF2 web files needed by React page components

This mode must let a Docusaurus app remove app-local localization sync scripts.

#### Deferred `hybrid`

Hybrid mode will combine literal static JSON for plain messages and WASM runtime
packs for dynamic messages. It is deferred until `plain-json` and `runtime` are
complete.

### Generated TypeScript Contract

Every mode that emits TypeScript must generate:

```ts
export type Locale = "en" | "es";
export type Namespace = "home" | "docs";
export type MessageId = "home.head.title" | "docs.introduction.title";

export type MessageArgsById = {
  "home.head.title": undefined;
  "cart.item_count": { count: number };
};

export type ArgsFor<K extends MessageId> =
  MessageArgsById[K] extends undefined ? undefined : MessageArgsById[K];
```

The generated format API must reject incorrect keys and argument shapes at
compile time for TypeScript users.

Web arg types:

- string
- number
- boolean
- datetime as `Date | { type: "datetime"; unixMilliseconds: number }`
- currency as `{ type: "currency"; value: number; code: string }`
- unit only after real browser unit formatting is implemented

### Browser WASM Runtime Contract

Upgrade `mf2_i18n_wasm` from a low-level parts constructor to a product runtime
surface.

Required exported surface:

- `Mf2Runtime.fromParts(...)`
- `Mf2Runtime.defaultLocale()`
- `Mf2Runtime.supportedLocales()`
- `Mf2Runtime.negotiateLocale(requested)`
- `Mf2Runtime.hasMessage(key)`
- `Mf2Runtime.format(locale, key, args)`

Required generated helper surface:

- `createMf2Runtime(options)`
- `loadLocale(locale)`
- `format(id, args, options?)`

Errors should be structured JS errors with stable codes:

- `MF2_INVALID_MANIFEST`
- `MF2_INVALID_ID_MAP`
- `MF2_MISSING_PACK`
- `MF2_PACK_HASH_MISMATCH`
- `MF2_MISSING_MESSAGE`
- `MF2_INVALID_ARGS`
- `MF2_UNSUPPORTED_FORMATTER`

### Docusaurus Contract

Docusaurus mode must be maximally native:

- Docusaurus should own routing and locale URL structure.
- Generated `i18n/<locale>/...` files should follow Docusaurus conventions.
- React page copy may use generated MF2 JSON or Docusaurus `Translate`, but
  the generated code must make the mapping deterministic.
- Docs MDX localization must preserve frontmatter and produce stable localized
  Docusaurus paths.
- Locale switching must not depend on app-local URL rewrites unless Docusaurus
  itself requires a theme override.

### Determinism and Stale Output Policy

All web generation must:

- sort locales, namespaces, message ids, and object keys deterministically
- write prettified JSON with final newlines
- stage generated files before replacing the destination
- produce a manifest of generated files
- fail `--check` when destination files differ
- optionally fail on stale generated files

Generated manifests should include:

- schema version
- generator version
- mode
- framework
- target name
- generated file list
- source config path
- default locale
- supported locales
- namespaces

### Validation

Each slice must keep these lanes green unless the slice explicitly narrows the
lane:

```sh
cargo fmt --all -- --check
cargo test
```

Generator slices must add focused golden-output tests. Runtime slices must add
WASM-facing Rust tests and, when JS fixture apps exist, browser tests.

### Downstream Confirmation Contract

This RCLD is not complete until the new web integration is proven in the
Radroots Platform Docusaurus docs site at `./domains/public/sites/dev/docs`
relative to the Radroots Platform repository root.

The downstream follow-up must:

- replace app-local MF2/Docusaurus sync scripts with generated artifacts from
  `mf2_i18n_cli build-web`
- keep the docs site using Docusaurus-native locale routing and generated
  Docusaurus i18n files
- preserve support for the `en` and `es` locales
- localize the homepage and all `docs/radroots/*.mdx` pages through generated
  outputs
- ensure Playwright tests derive expectations from generated MF2, generated
  Docusaurus artifacts, and source MDX rather than hardcoded translated prose
- validate the locale switcher does not append repeated locale path segments
- validate the Radroots command path starts the production-like multi-locale
  docs site

Required downstream validation lanes:

```sh
pnpm --dir domains/public/sites/dev/docs run test:site:docusaurus
pnpm --dir domains/public/sites/dev/docs run test:site:radroots-scripts
j rs local public-site docs check
```

## Non-Goals For This RCLD

- publishing npm packages
- adding Node-based generation
- replacing the Rust runtime with a JS interpreter
- implementing every JS framework in the first production slice
- adding JS extraction before the output and runtime contracts stabilize

## Slice Map

### RCLD-01: Web Target Config Model

Add web target config structures, TOML loading, validation errors, and tests.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build config
```

### RCLD-02: Web Artifact Manifest And Atomic Writer

Add generated-file manifest types plus staged write, atomic replace, `--check`,
and stale-file policy helpers.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build web
```

### RCLD-03: Plain JSON Web Target

Move the current `export-web-json` behavior behind the new `plain-json` web
target and emit schemaed manifest, `types.ts`, `loaders.ts`, and `index.ts`.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build web_json
cargo test -p mf2_i18n_cli export_web_json
```

### RCLD-04: `build-web` CLI

Add `mf2_i18n_cli build-web --target <name> --check` and wire it to the new
web build API.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_cli
```

### RCLD-05: Typed Message Metadata Generator

Generate `Locale`, `Namespace`, `MessageId`, `MessageArgsById`, and `ArgsFor`
from catalog metadata and parsed MF2 message requirements.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build web
```

### RCLD-06: Runtime Web Target Assets

Generate runtime-mode `manifest.json`, `id-map.json`, `packs/*.mf2pack`,
`runtime-assets.json`, and loader TypeScript from the existing runtime artifact
pipeline.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build project_pipeline web
```

### RCLD-07: WASM Runtime API Hardening

Add `negotiateLocale`, `hasMessage`, stable error codes, and generated-friendly
argument conversion. Remove or gate web unit args until browser unit formatting
is real.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_wasm
cargo test -p mf2_i18n_runtime
```

### RCLD-08: Generated Runtime Loader API

Generate `createMf2Runtime`, `loadLocale`, `format`, cache semantics, and
browser-safe asset loading for runtime mode.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build web
```

### RCLD-09: Docusaurus Native Target

Generate Docusaurus `code.json`, theme translation JSON, docs plugin metadata,
and localized MDX from MF2 catalogs.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build docusaurus
```

### RCLD-10: Fixture Site Coverage

Add fixture projects for plain HTML, React or Vite, and Docusaurus output. Keep
tests source-driven rather than hardcoding translated prose.

Green lane:

```sh
cargo fmt --all -- --check
cargo test
```

### RCLD-11: Radroots Docs Downstream Integration

Replace the Radroots Platform docs site's app-local MF2/Docusaurus generation
scripts with the new `mf2_i18n_cli build-web` target, then validate the
homepage, all Radroots docs pages, locale switching, and the command-level
production-like docs server through Playwright.

Green lane from the Radroots Platform repository root:

```sh
pnpm --dir domains/public/sites/dev/docs run test:site:docusaurus
pnpm --dir domains/public/sites/dev/docs run test:site:radroots-scripts
j rs local public-site docs check
```

### RCLD-12: Documentation And Migration Guide

Update `README.md` and `docs/integration.md` with the web target contract,
examples, CLI usage, migration from `export-web-json`, and the downstream
Radroots Docusaurus docs validation workflow.

Green lane:

```sh
cargo fmt --all -- --check
cargo test
```

### RCLD-13: Deferred Hybrid Planning Gate

Re-review implemented `plain-json`, `runtime`, `docusaurus`, and Radroots docs
downstream surfaces, then define the separate hybrid-mode RCLD or append a
second phase if the contract is stable.

Green lane:

```sh
cargo fmt --all -- --check
cargo test
```
