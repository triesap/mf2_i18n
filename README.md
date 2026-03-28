# mf2-i18n

Unicode MessageFormat v2 (MF2) i18n for Rust, with a no_std core and portable runtimes.

## Goals

- Provide a deterministic MF2 runtime with no runtime parsing.
- Use downloadable, content-addressed language packs.
- Support micro-locales via overlay packs and clear fallback chains.
- Target wasm, server, and embedded environments.

## Crates

- `mf2-i18n-core`: no_std MF2 execution core and pack model.
- `mf2-i18n-runtime`: filesystem-backed runtime for manifest/id-map/pack artifacts.
- `mf2-i18n-embedded`: embedded runtime for in-binary pack delivery.
- `mf2-i18n-native`: native bridge/runtime helpers for Apple and other std clients.
- `mf2-i18n-build`: supported build/project API for extraction, validation, compilation, id-map generation, and pack generation.
- `mf2-i18n-cli`: thin CLI over `mf2-i18n-build`.

## Integration paths

`mf2-i18n` supports three public integration paths:

- filesystem runtime: use `mf2-i18n-runtime` when the host loads `manifest.json`, `id-map.json`, and `.mf2pack` files from disk
- embedded runtime: use `mf2-i18n-embedded` when the host ships packs in-binary
- native bridge: use `mf2-i18n-native` when the host needs locale negotiation and a Rust-owned runtime surface

For std targets, `mf2-i18n-runtime::Runtime::format()` and `mf2-i18n-native::NativeLocalizer::format()` use `mf2-i18n-std::StdFormatBackend` by default. `mf2-i18n-embedded::EmbeddedRuntime::format()` still defaults to `UnsupportedFormatBackend`; use `format_with_backend(...)` when an embedded host needs locale-sensitive plural, number, date, time, datetime, unit, or currency output.

`mf2-i18n-native` keeps strict translation and key-fallback convenience separate. Use `tr(...)` or `tr_with_args(...)` when the caller wants a `Result`, and use `tr_or_key(...)` or `tr_with_args_or_key(...)` when key fallback is intentional.

`BasicFormatBackend` remains available for diagnostics and simple tests. It does not provide locale-sensitive output.

Datetime arguments use `mf2_i18n_core::DateTimeValue`, with explicit `unix_seconds(...)` and `unix_milliseconds(...)` constructors. Runtime formatting does not infer timestamp units from magnitude.

Formatter calls may include literal named options such as `{ $total :number style=percent minimum-fraction-digits=2 }`. The build pipeline preserves those options in compiled artifacts and passes them through to runtime format backends.

`StdFormatBackend` resolves locale data only from the requested locale chain. It does not silently fall back to unrelated global defaults. Use `StdFormatBackend::resolution()` to inspect the requested locale, plural locale, number locale, and date locale that were resolved.

`StdFormatBackend` formats currency values with locale-sensitive decimal output and ISO 4217 code display. The supported currency option surface is `display=code` or no display option. Unit formatting does not invent labels from opaque `unit_id` values; it returns an explicit unsupported error until a unit label source is available.

Build outputs for generated native runtimes use `platform-bundle.json` as the stable entry point over `manifest.json`, `id-map.json`, and `.mf2pack` files. Code generators should target `mf2_i18n_build::PlatformBundle` and the bundle sidecar instead of stitching those files together ad hoc.

See [docs/runtime-integration.md](docs/runtime-integration.md) for the runtime decision guide and [docs/platform-consumption.md](docs/platform-consumption.md) for generated-runtime inputs.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE`.
