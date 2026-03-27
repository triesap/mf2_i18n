# platform consumption

`mf2-i18n` supports two library-owned consumption modes.

## bridge runtime

Use the Rust runtimes directly when the host application can call into Rust:

- `mf2-i18n-runtime` for filesystem-backed loading
- `mf2-i18n-embedded` for embedded pack delivery
- `mf2-i18n-native` for native clients that need active locale management and preferred-locale negotiation

This mode keeps message evaluation inside the Rust runtime surface.

## generated native runtime

Use the build-side platform bundle when generating native client code or resources.

The build output now contains:

- `manifest.json`
- `id-map.json`
- `packs/*.mf2pack`
- `platform-bundle.json`

`platform-bundle.json` is the codegen entry point. It contains the runtime manifest plus the relative `id-map.json` path. Generators should load it through `mf2_i18n_build::PlatformBundle` or `load_platform_bundle_manifest(...)` instead of reading individual files ad hoc.

This keeps code generators anchored to the same message ids, locale topology, pack hashes, and pack bytes that the Rust runtime uses.

## generator guidance

- treat `platform-bundle.json` as the root input for generated native code
- keep `id-map.json` and `.mf2pack` files authoritative
- do not derive new message ids in platform generators
- do not rewrite locale fallback rules in app code
- if a native target can call the Rust bridge directly, prefer the bridge runtime mode
