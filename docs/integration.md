# integration

`mf2_i18n` is the main crate most applications should use for both build-time
and runtime integration.

If you need direct access to one layer, the lower-level workspace crates remain
available, but they are not the default starting point.

## choose a feature

- no features: core MF2 types, values, and locale negotiation helpers
- `runtime`: filesystem runtime plus `mf2_i18n::StdFormatBackend`
- `embedded`: embedded runtime for in-binary id maps and packs
- `native`: native localizer and `define_i18n_module!`; includes `runtime`,
  `embedded`, and `std_backend`
- `build`: project-oriented build APIs for `build.rs`
- `std_backend`: direct access to `mf2_i18n::StdFormatBackend`

## build from `build.rs`

Use `mf2_i18n = { features = ["build"] }` when your application owns
checked-in locale catalogs, a checked-in `mf2_i18n.toml` project file, and a
checked-in `id_salt.txt` file beside that config.

Main build entry points:

- `mf2_i18n::build::build_native_module(...)`
- `mf2_i18n::build::NativeModuleBuildOptions`
- `mf2_i18n::build::build_project_runtime_artifacts(...)`
- `mf2_i18n::build::ProjectRuntimeBuildOptions`

The native-module pipeline writes:

- `manifest.json`
- `id-map.json`
- `id-map.sha256`
- `packs/*.mf2pack`
- `platform-bundle.json`
- `generated_module.rs`
- `generated_catalog.rs`

The returned build output also exposes `rerun_if_changed_paths()` so `build.rs`
can emit deterministic Cargo rebuild triggers.

## choose a runtime path

Use `mf2_i18n::Runtime` when the host loads `manifest.json`, `id-map.json`,
and `.mf2pack` files from disk.

Use `mf2_i18n::EmbeddedRuntime` when the host ships id maps and pack bytes
in-binary.

Use `mf2_i18n::NativeLocalizer` when the host wants Rust-owned locale
negotiation and translation helpers.

On std targets:

- `Runtime::format(...)` uses `mf2_i18n::StdFormatBackend`
- `NativeLocalizer::format(...)` uses `mf2_i18n::StdFormatBackend`
- `EmbeddedRuntime` requires `format_with_backend(...)` for locale-sensitive
  formatting

## integrate with JavaScript

JavaScript apps should use named web targets in `mf2_i18n.toml`. The Rust
tooling stays the source of truth; generated JavaScript, TypeScript, JSON, MF2
packs, and Docusaurus files are build outputs.

The minimum target shape is:

```toml
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "site"
mode = "runtime"
framework = "vite"
out_dir = "src/generated/mf2"
locales = ["en", "es"]
namespaces = ["home", "checkout"]
release_id = "site-local"
generated_at = "2026-02-01T00:00:00Z"
stale_policy = "fail"
```

JavaScript projects should generate and verify web targets through the npm
launcher package:

```json
{
  "devDependencies": {
    "@mf2-i18n/cli": "0.1.0"
  },
  "scripts": {
    "mf2:gen": "mf2-i18n build-web --config i18n/mf2_i18n.toml --target site",
    "mf2:check": "mf2-i18n check-web --config i18n/mf2_i18n.toml --target site"
  }
}
```

Then run:

```sh
npm run mf2:gen
npm run mf2:check
```

`build-web` reads the named target, validates the target-specific required
fields, generates deterministic outputs, writes `mf2-web-manifest.json`, and
honors the target's stale-file policy.

`check-web` delegates to the same Rust `build-web` command with check mode
enabled. Direct Rust source checkouts may still use
`cargo run -p mf2_i18n_cli -- build-web --config i18n/mf2_i18n.toml --target site`
or add `--check` for verification, but application package scripts should not
carry app-local workspace-discovery code.

The launcher resolves the Rust CLI in this order:

- `--cli-bin`
- `MF2_I18N_CLI_BIN`
- nearest `package.json` `mf2_i18n.cliBin`
- package-local native binary
- `--manifest-path`
- `MF2_I18N_CARGO_MANIFEST_PATH`
- nearest `package.json` `mf2_i18n.cargoManifestPath`
- adjacent source checkout

`mf2-i18n doctor` reports the selected resolution path without generating
outputs.

### plain JSON target

Use `plain-json` when the app only needs literal localized strings and does not
need the MF2 runtime in the browser:

```toml
[[web.targets]]
name = "static-copy"
mode = "plain-json"
framework = "vanilla"
out_dir = "src/generated/mf2"
locales = ["en", "es"]
namespaces = ["home"]
stale_policy = "fail"
```

Generated outputs include:

- `mf2-web-manifest.json`
- `metadata.json`
- `types.ts`
- `loaders.ts`
- `index.ts`
- `messages/<locale>/<namespace>.json`

Plain mode is intentionally limited to literal text. Variables, formatters,
select messages, and plural messages fail generation before output is accepted.

### runtime target

Use `runtime` when browser or bundler code needs full MF2 behavior:

```toml
[[web.targets]]
name = "app-runtime"
mode = "runtime"
framework = "vite"
out_dir = "src/generated/mf2"
locales = ["en", "es"]
namespaces = ["home", "checkout"]
release_id = "app-local"
generated_at = "2026-02-01T00:00:00Z"
stale_policy = "fail"
```

Generated outputs include:

- `mf2-web-manifest.json`
- `manifest.json`
- `platform-bundle.json`
- `id-map.json`
- `id-map.sha256`
- `metadata.json`
- `runtime-assets.json`
- `packs/<locale>.mf2pack`
- `types.ts`
- `loaders.ts`
- `index.ts`

The generated TypeScript surface includes typed locale, namespace, and message
id unions, message argument metadata, runtime asset metadata, pack loading,
caching helpers, `createMf2Runtime(...)`, `loadLocale(...)`, and `format(...)`.

The JavaScript runtime constructor expects one object:

```ts
import init, { Mf2Runtime } from "./mf2_i18n_wasm";

await init();

const runtime = Mf2Runtime.fromParts({
  manifest,
  idMap,
  packs: {
    en: enPackBytes,
    es: esPackBytes,
  },
});
```

`manifest` is the parsed or stringified `manifest.json`, `idMap` is the parsed
or binary `id-map.json`, and each pack value is a `Uint8Array` or `ArrayBuffer`
loaded from `packs/<locale>.mf2pack`. Browser formatting uses `Intl` for plural
selection, number formatting, date/time formatting, and currency formatting.

### hybrid target

Use `hybrid` when an app has a mix of literal copy and dynamic MF2 messages but
should expose one generated localization API to application code:

```toml
[[web.targets]]
name = "app-hybrid"
mode = "hybrid"
framework = "vite"
out_dir = "src/generated/mf2"
locales = ["en", "es"]
namespaces = ["home", "checkout"]
release_id = "app-local"
generated_at = "2026-02-01T00:00:00Z"
stale_policy = "fail"
```

Generated outputs include:

- `mf2-web-manifest.json`
- `hybrid-assets.json`
- `runtime-assets.json`
- `manifest.json`
- `platform-bundle.json`
- `id-map.json`
- `id-map.sha256`
- `metadata.json`
- `packs/<locale>.mf2pack`
- `messages/<locale>/<namespace>.json`
- `types.ts`
- `loaders.ts`
- `index.ts`

The generator classifies each default-locale message. Literal messages without
arguments are written to static JSON. Messages with variables, formatters,
selects, plurals, dates, numbers, or currencies are routed through the runtime
pack path.

Generated TypeScript includes `MessageId`, `ArgsFor<K>`, locale and namespace
unions, `HYBRID_ASSETS`, `RUNTIME_ASSETS`, `STATIC_MESSAGE_LOADERS`,
`isStaticMessage(...)`, `isRuntimeMessage(...)`, `loadStaticMessages(...)`,
`loadRuntimeParts(...)`, `createMf2Runtime(...)`, and one async `format(...)`.

Application code passes the WASM runtime class only to the generated surface:

```ts
import init, { Mf2Runtime } from "./mf2_i18n_wasm";
import { format } from "./generated/mf2";

await init();

const title = await format(Mf2Runtime, "home.title", {}, { locales: "es" });
const count = await format(
  Mf2Runtime,
  "checkout.itemCount",
  { count: 3 },
  { locales: ["es-MX", "es"] },
);
```

Static messages load through generated JSON imports. Runtime assets are fetched
and cached only when a dynamic message is formatted.

Use hybrid instead of maintaining separate `plain-json` and `runtime` targets
for the same application surface. Migration should collapse app code onto the
hybrid `format(...)` call, delete duplicate app-local loader branches, keep one
checked-in generated output directory, and verify it with
`build-web --target <name> --check`.

### Docusaurus target

Use `docusaurus` when a Docusaurus site should keep Docusaurus-native locale
routing and translation files while sourcing strings and localized MDX from MF2
sources:

```toml
[[web.targets]]
name = "docs"
mode = "docusaurus"
framework = "docusaurus"
out_dir = "src/generated/mf2"
docs_source_dir = "docs"
docusaurus_i18n_dir = "i18n"
docusaurus_sidebar_id = "docsSidebar"
locales = ["en", "es"]
namespaces = ["home", "docs", "docusaurus"]
stale_policy = "fail"
```

Generated outputs include:

- `src/generated/mf2/messages.ts`
- `src/generated/mf2/mf2-web-manifest.json`
- `i18n/<locale>/code.json`
- `i18n/<locale>/docusaurus-theme-classic/navbar.json`
- `i18n/<locale>/docusaurus-plugin-content-docs/current.json`
- `i18n/<locale>/docusaurus-plugin-content-docs/current/**/*.mdx`
- `i18n/.mf2-managed.json`

Docusaurus namespace keys map to Docusaurus files by convention:

- `docusaurus.navbar.<id>` writes navbar translation ids such as
  `item.label.<Label>` and `logo.alt`
- `docusaurus.docs.sidebar.<id>` writes docs sidebar translation ids such as
  `sidebar.<sidebar id>.category.<Label>`
- all other namespaces are treated as React page/component copy and written to
  `code.json` plus `messages.ts`

Default-locale docs are authored directly under `docs_source_dir`. Non-default
localized docs are authored as `locales/<locale>/docs/**/*.mf2` files. Each doc
file must define one `content = ...` entry; everything after that entry line is
preserved as MDX, including blank lines. The relative file path under
`docs/` maps to the generated Docusaurus MDX path, so
`locales/es/docs/guide/intro.mf2` generates
`i18n/es/docusaurus-plugin-content-docs/current/guide/intro.mdx`.
Flat `locales/<locale>/docs.mf2` catalogs are rejected for Docusaurus docs.

Docusaurus should still own routing, static builds, sidebar metadata, and
`<html lang>`. App-local sync scripts should be removed once the generated
Docusaurus target owns the files.

### migrate from `export-web-json`

`export-web-json` remains available for literal-only JSON output, but new web
integrations should use named `build-web` targets through `@mf2-i18n/cli`.

Migration steps:

1. Add one or more `[[web.targets]]` entries to `mf2_i18n.toml`.
2. Replace `mf2_i18n_cli export-web-json --out <dir> --mode plain` with
   a package script that runs `mf2-i18n build-web --target <name>`.
3. Replace app-local TypeScript loader handwork with generated `types.ts`,
   `loaders.ts`, and `index.ts`.
4. For mixed literal and dynamic web copy, use one `hybrid` target instead of
   separate plain/runtime outputs for the same UI.
5. For Docusaurus, delete custom code-json and localized-MDX sync scripts and
   let `build-web --target docs` write Docusaurus-native outputs.
6. Add `mf2-i18n check-web --target <name>` to CI or local verification when
   the generated outputs are checked in.

### downstream Docusaurus proof

The Radroots Platform docs site is the reference downstream Docusaurus
integration. From the Radroots Platform repository root, the known-green
validation lanes are:

```sh
pnpm --dir domains/public/sites/dev/docs run test:site:docusaurus
pnpm --dir domains/public/sites/dev/docs run test:site:radroots-scripts
j rs local public-site docs check
```

Those tests verify the generated MF2 React page copy, generated Docusaurus
translation files, localized `docs/radroots/*.mdx` pages, canonical locale
switching without repeated locale path segments, and the production-like
`radroots-scripts local public-site docs run` server path.

## generated native bundles

Use `mf2_i18n::PlatformBundle` when a generator or native host consumes the
compiled outputs instead of calling the Rust runtime directly.

Bundle-oriented outputs include:

- `manifest.json`
- `id-map.json`
- `packs/*.mf2pack`
- `platform-bundle.json`

Treat `platform-bundle.json` as the root input for generators. Keep `id-map`
and pack files authoritative, keep paths relative to the bundle root, and do
not derive new message ids outside the build pipeline.

## lower-level crates

Reach for the lower-level crates only when the main crate is not the right fit
for your integration boundary:

- `mf2_i18n_core`: core MF2 execution types and primitives
- `mf2_i18n_build`: lower-level build and project APIs
- `mf2_i18n_runtime`: filesystem runtime internals
- `mf2_i18n_embedded`: embedded runtime internals
- `mf2_i18n_native`: native runtime helpers and macro surface
- `mf2_i18n_std`: std-target formatting backend
- `mf2_i18n_cli`: command-line tooling
