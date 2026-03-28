# runtime integration

`mf2-i18n` supports three runtime integration paths and one generated-runtime input contract.

## filesystem runtime

Use `mf2-i18n-runtime` when the host can load build artifacts from disk and evaluate messages in Rust.

- input files: `manifest.json`, `id-map.json`, `packs/*.mf2pack`
- entry point: `mf2_i18n_runtime::Runtime`
- locale-sensitive formatting: `Runtime::format(...)` uses `mf2-i18n-std::StdFormatBackend` by default
- currency semantics: locale-sensitive decimal formatting with ISO 4217 code display
- unit semantics: explicit unsupported error until a unit label source is provided
- locale fallback visibility: `StdFormatBackend::resolution()` reports the requested locale and the resolved plural, number, and date locales

This is the direct runtime path for servers, desktop tools, and std-target applications that can keep message evaluation inside Rust.

## embedded runtime

Use `mf2-i18n-embedded` when the host ships the id map and pack bytes in-binary.

- input values: embedded `id-map` entries, pack bytes, and a default locale
- entry point: `mf2_i18n_embedded::EmbeddedRuntime`
- locale-sensitive formatting: call `format_with_backend(...)` with a `FormatBackend`

`EmbeddedRuntime::format(...)` intentionally returns an error for plural, number, date, time, datetime, unit, and currency formatting. On std targets, the supported library-owned backend is `mf2_i18n_std::StdFormatBackend`.

## native bridge

Use `mf2-i18n-native` when a native client wants Rust-owned locale negotiation and message evaluation.

- entry point: `mf2_i18n_native::NativeLocalizer`
- locale negotiation: `set_preferred_locales(...)`
- locale-sensitive formatting: `NativeLocalizer::format(...)` uses `mf2-i18n-std::StdFormatBackend` by default
- strict translation: `NativeLocalizer::tr(...)` and `NativeLocalizer::tr_with_args(...)`
- key fallback convenience: `NativeLocalizer::tr_or_key(...)` and `NativeLocalizer::tr_with_args_or_key(...)`
- generated module setup: `define_i18n_module!` with `init_policy: strict` or `init_policy: fallback_to_keys`

This is the bridge path for Apple and other std-target native hosts that can call Rust directly.

## generated runtime inputs

Use `platform-bundle.json` when a host generates native code or resources instead of calling the Rust runtime directly.

- entry point: `mf2_i18n_build::PlatformBundle`
- sidecar files: `id-map.json`, `packs/*.mf2pack`
- bundle rule: all file references stay relative to the bundle root

The generated runtime path is responsible for consuming the same ids, locale topology, and pack bytes that the Rust runtime uses.

## backend selection

- use `StdFormatBackend` for locale-sensitive std-target formatting
- use `BasicFormatBackend` only for simple debug-style output
- do not treat `BasicFormatBackend` as a locale-sensitive backend
- `StdFormatBackend` resolves data only from the requested locale chain and returns explicit unsupported errors when a formatter class has no data for that chain
- `StdFormatBackend` supports `display=code` for currency formatting and rejects other currency display modes explicitly
- `StdFormatBackend` does not synthesize unit labels from numeric `unit_id` values
