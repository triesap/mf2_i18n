# platform consumption

Most applications should use the `mf2_i18n` façade for both build-time and
runtime integration. The lower-level build crate remains available for advanced
callers, but it is not the default consumer path.

## canonical app build path

Use `mf2_i18n = { features = ["build"] }` from `build.rs` when the application
owns checked-in locale catalogs and wants one canonical project pipeline.

The façade build entry point is:

- `mf2_i18n::build::build_native_module(...)`
- options type: `mf2_i18n::build::NativeModuleBuildOptions`
- project config: `mf2_i18n.toml`
- locale inputs: `messages.json` files grouped by locale directory

The native-module pipeline writes:

- `id-map.json`
- `id-map.sha256`
- `*.mf2pack`
- `generated_module.rs`
- `generated_catalog.rs`

The generated module defaults to `mf2_i18n::define_i18n_module!`. If an
advanced integration needs the expert macro path instead, use
`with_module_macro_path(...)`.

The build output also returns `rerun_if_changed_paths()` so `build.rs` can emit
deterministic Cargo rebuild triggers without reimplementing the project scan.

## direct Rust runtime path

Use the Rust runtimes directly when the host application can call into Rust:

- `mf2_i18n::Runtime` for filesystem-backed loading
- `mf2_i18n::EmbeddedRuntime` for embedded pack delivery
- `mf2_i18n::NativeLocalizer` for native clients that need active locale
  management and preferred-locale negotiation

This mode keeps message evaluation inside the Rust runtime surface.

For locale-sensitive output on std targets:

- `mf2_i18n::Runtime::format(...)` uses `mf2_i18n::StdFormatBackend`
- `mf2_i18n::NativeLocalizer::format(...)` uses `mf2_i18n::StdFormatBackend`
- `mf2_i18n::EmbeddedRuntime` requires `format_with_backend(...)`

## generated native runtime

Use the build-side platform bundle when generating native client code or
resources instead of calling the Rust runtime directly.

The bundle-oriented output contains:

- `manifest.json`
- `id-map.json`
- `packs/*.mf2pack`
- `platform-bundle.json`

`platform-bundle.json` is the codegen entry point. It contains the runtime
manifest plus the relative `id-map.json` path. Generators should load it
through `mf2_i18n::PlatformBundle` or the namespaced `mf2_i18n::build::*`
surface instead of reading individual files ad hoc.

Bundle file references are bundle-local. `id-map.json` and pack paths must stay
relative to the bundle root and must not use absolute paths or parent
traversal.

This keeps code generators anchored to the same message ids, locale topology,
pack hashes, and pack bytes that the Rust runtime uses.

If a generated host later needs locale-sensitive formatting semantics, it
should use the same locale-aware rules as the Rust std backend instead of
treating `BasicFormatBackend` output as canonical.

## generator guidance

- treat `platform-bundle.json` as the root input for generated native code
- keep `id-map.json` and `.mf2pack` files authoritative
- do not derive new message ids in platform generators
- do not rewrite locale fallback rules in app code
- if a native target can call the Rust bridge directly, prefer the façade
  runtime mode before dropping to expert crates

## expert escape hatches

Use the lower-level crates directly only when the façade surface is not the
right fit for a specific integration boundary:

- `mf2_i18n_build` for crate-local build and bundle modules
- `mf2_i18n_runtime` for direct runtime internals
- `mf2_i18n_embedded` for direct embedded runtime internals
- `mf2_i18n_native` for direct native bridge internals
