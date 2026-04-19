# platform consumption

`mf2_i18n` supports a Rust bridge path and a generated-runtime input contract.

## bridge runtime

Use the Rust runtimes directly when the host application can call into Rust:

- `mf2_i18n_runtime` for filesystem-backed loading
- `mf2_i18n_embedded` for embedded pack delivery
- `mf2_i18n_native` for native clients that need active locale management and preferred-locale negotiation

This mode keeps message evaluation inside the Rust runtime surface.

For locale-sensitive output on std targets:

- `mf2_i18n_runtime::Runtime::format(...)` uses `mf2_i18n_std::StdFormatBackend`
- `mf2_i18n_native::NativeLocalizer::format(...)` uses `mf2_i18n_std::StdFormatBackend`
- `mf2_i18n_embedded::EmbeddedRuntime` requires `format_with_backend(...)`

## generated native runtime

Use the build-side platform bundle when generating native client code or resources.

The build output now contains:

- `manifest.json`
- `id-map.json`
- `packs/*.mf2pack`
- `platform-bundle.json`

`platform-bundle.json` is the codegen entry point. It contains the runtime manifest plus the relative `id-map.json` path. Generators should load it through `mf2_i18n_build::PlatformBundle` or `load_platform_bundle_manifest(...)` instead of reading individual files ad hoc.

Bundle file references are bundle-local. `id-map.json` and pack paths must stay relative to the bundle root and must not use absolute paths or parent traversal.

This keeps code generators anchored to the same message ids, locale topology, pack hashes, and pack bytes that the Rust runtime uses.

If a generated host later needs locale-sensitive formatting semantics, it should use the same locale-aware rules as the Rust std backend instead of treating `BasicFormatBackend` output as canonical.

## generator guidance

- treat `platform-bundle.json` as the root input for generated native code
- keep `id-map.json` and `.mf2pack` files authoritative
- do not derive new message ids in platform generators
- do not rewrite locale fallback rules in app code
- if a native target can call the Rust bridge directly, prefer the bridge runtime mode
