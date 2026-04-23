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

JavaScript apps have two supported integration paths.

Use static web JSON when the app only needs literal localized strings and does
not need the MF2 runtime in the browser:

```sh
cargo run -p mf2_i18n_cli -- export-web-json \
  --config i18n/mf2_i18n.toml \
  --out web-i18n \
  --mode plain
```

This writes:

- `web-i18n/messages/<locale>/<namespace>.json`
- `web-i18n/i18n-manifest.ts`

The generated TypeScript manifest exports `DEFAULT_LOCALE`,
`SUPPORTED_LOCALES`, `MESSAGE_NAMESPACES`, and `MESSAGE_LOADERS`. Plain mode is
intentionally limited to literal text; variables, formatters, select messages,
and plural messages fail the export.

Use the WASM runtime when browser or bundler code needs full MF2 behavior:

```sh
cargo run -p mf2_i18n_cli -- build \
  --config i18n/mf2_i18n.toml \
  --out i18n-runtime \
  --release-id app-local \
  --generated-at 2026-02-01T00:00:00Z
scripts/package-web.sh all
```

The runtime build writes `manifest.json`, `id-map.json`, and
`packs/<locale>.mf2pack`. `scripts/package-web.sh web` writes the browser ESM
package to `pkg/mf2_i18n_wasm-web`; `scripts/package-web.sh bundler` writes the
bundler ESM package to `pkg/mf2_i18n_wasm-bundler`; `all` writes both.

The JavaScript runtime constructor expects one object:

```ts
import init, { Mf2Runtime } from "./pkg/mf2_i18n_wasm-web/mf2_i18n_wasm.js";

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
