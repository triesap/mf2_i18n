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

`mf2-i18n-runtime::Runtime::format()` uses `mf2-i18n-std::StdFormatBackend` by default for std-target locale-sensitive formatting. `mf2-i18n-embedded::EmbeddedRuntime::format()` still uses `UnsupportedFormatBackend` by default, and callers can provide a backend with `format_with_backend(...)` when they need formatter support in embedded contexts.

## Native helpers

`mf2-i18n-native` owns preferred-locale negotiation and active locale selection for native clients. `NativeLocalizer::set_preferred_locales(...)` negotiates against the available locales, `NativeLocalizer::format(...)` uses the std backend by default, and `define_i18n_module!` requires an explicit `init_policy: strict` or `init_policy: fallback_to_keys` choice for embedded artifacts.

## Platform bundles

Build outputs for generated native runtimes use `platform-bundle.json` as the stable entry point over `manifest.json`, `id-map.json`, and `.mf2pack` files. Code generators should target `mf2_i18n_build::PlatformBundle` and the bundle sidecar instead of stitching those files together ad hoc.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE`.
