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

## Runtime backends

`mf2-i18n-runtime::Runtime::format()` and `mf2-i18n-embedded::EmbeddedRuntime::format()` use `UnsupportedFormatBackend` by default. Plain text and identity output work without extra setup, while plural, number, date, time, datetime, unit, and currency formatting require `format_with_backend(...)` and a caller-provided `FormatBackend`.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE`.
