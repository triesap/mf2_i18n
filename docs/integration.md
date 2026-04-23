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
