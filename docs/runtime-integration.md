# runtime integration

Most applications should depend on the feature-gated `mf2_i18n` façade and use
its root re-exports or namespaced feature modules. The lower-level runtime
crates remain available for advanced callers that want to bind directly to a
specific layer.

## façade feature map

- no features: core MF2 types, values, and locale negotiation helpers only
- `runtime`: `mf2_i18n::Runtime`, runtime manifest/id-map helpers, and
  `mf2_i18n::StdFormatBackend`
- `embedded`: `mf2_i18n::EmbeddedRuntime` and `mf2_i18n::EmbeddedPack`
- `native`: `mf2_i18n::NativeLocalizer`, `mf2_i18n::define_i18n_module!`, and
  the runtime plus std-backend surface needed by that native path
- `build`: generated-runtime build and codegen helpers such as
  `mf2_i18n::PlatformBundle`

## filesystem runtime

Use `mf2_i18n = { features = ["runtime"] }` when the host can load build
artifacts from disk and evaluate messages in Rust.

- input files: `manifest.json`, `id-map.json`, `packs/*.mf2pack`
- façade entry point: `mf2_i18n::Runtime`
- locale-sensitive formatting: `Runtime::format(...)` uses
  `mf2_i18n::StdFormatBackend` by default
- currency semantics: locale-sensitive decimal formatting with ISO 4217 code display
- unit semantics: explicit unsupported error until a unit label source is provided
- locale fallback visibility: `StdFormatBackend::resolution()` reports the requested locale and the resolved plural, number, and date locales

This is the direct runtime path for servers, desktop tools, and std-target applications that can keep message evaluation inside Rust.

Advanced callers can still target `mf2_i18n_runtime` directly when they want
crate-local module access rather than the façade surface.

## embedded runtime

Use `mf2_i18n = { features = ["embedded"] }` when the host ships the id map and
pack bytes in-binary.

- input values: embedded `id-map` entries, pack bytes, and a default locale
- façade entry point: `mf2_i18n::EmbeddedRuntime`
- locale-sensitive formatting: call `format_with_backend(...)` with a `FormatBackend`

`EmbeddedRuntime::format(...)` intentionally returns an error for plural,
number, date, time, datetime, unit, and currency formatting. On std targets,
the supported library-owned backend is `mf2_i18n::StdFormatBackend`.

Advanced callers can still target `mf2_i18n_embedded` directly when they want
only the embedded layer.

## native bridge

Use `mf2_i18n = { features = ["native"] }` when a native client wants
Rust-owned locale negotiation and message evaluation.

- façade entry point: `mf2_i18n::NativeLocalizer`
- locale negotiation: `set_preferred_locales(...)`
- locale-sensitive formatting: `NativeLocalizer::format(...)` uses
  `mf2_i18n::StdFormatBackend` by default
- strict translation: `NativeLocalizer::tr(...)` and `NativeLocalizer::tr_with_args(...)`
- key fallback convenience: `NativeLocalizer::tr_or_key(...)` and `NativeLocalizer::tr_with_args_or_key(...)`
- generated module setup: `mf2_i18n::define_i18n_module!` with
  `init_policy: strict` or `init_policy: fallback_to_keys`

This is the bridge path for Apple and other std-target native hosts that can call Rust directly.

Advanced callers can still target `mf2_i18n_native` directly when they need the
expert crate surface or want to bypass the façade.

## generated runtime inputs

Use `mf2_i18n = { features = ["build"] }` when a host generates native code or
resources instead of calling the Rust runtime directly.

- façade entry point: `mf2_i18n::PlatformBundle`
- sidecar files: `id-map.json`, `packs/*.mf2pack`
- bundle rule: all file references stay relative to the bundle root

The generated runtime path is responsible for consuming the same ids, locale topology, and pack bytes that the Rust runtime uses.

Advanced callers can still target `mf2_i18n_build` directly for low-level
build, validation, or bundle modules.

## backend selection

- use `mf2_i18n::StdFormatBackend` for locale-sensitive std-target formatting
- use `mf2_i18n::runtime::BasicFormatBackend` or
  `mf2_i18n::embedded::BasicFormatBackend` only for simple debug-style output
- do not treat `BasicFormatBackend` as a locale-sensitive backend
- `StdFormatBackend` resolves data only from the requested locale chain and returns explicit unsupported errors when a formatter class has no data for that chain
- `StdFormatBackend` supports `display=code` for currency formatting and rejects other currency display modes explicitly
- `StdFormatBackend` does not synthesize unit labels from numeric `unit_id` values
