# mf2_i18n

Unicode MessageFormat v2 (MF2) i18n for Rust, with a no_std core and portable runtimes.

## Goals

- Provide a deterministic MF2 runtime with no runtime parsing.
- Use downloadable, content-addressed language packs.
- Support micro-locales via overlay packs and clear fallback chains.
- Target wasm, server, and embedded environments.

## Start Here

Most consumers should depend on `mf2_i18n` and enable only the features they
need.

```toml
[dependencies]
mf2_i18n = { version = "0.1.0", features = ["native"] }

[build-dependencies]
mf2_i18n = { version = "0.1.0", features = ["build"] }
```

Feature surface:

- no features: core MF2 types, values, and locale negotiation helpers
- `runtime`: filesystem runtime plus `StdFormatBackend`
- `embedded`: embedded pack runtime
- `native`: native localizer and `define_i18n_module!`; implies `runtime`,
  `embedded`, and `std_backend`
- `build`: project-oriented build pipeline and platform bundle types
- `std_backend`: explicit access to `StdFormatBackend` when `runtime` or
  `native` are not enabled

Most application integrations fall into one of these two paths:

- runtime path: enable `native`, `runtime`, or `embedded` and use the façade
  entry points such as `mf2_i18n::NativeLocalizer`, `mf2_i18n::Runtime`, or
  `mf2_i18n::EmbeddedRuntime`
- build path: enable `build` and use `mf2_i18n::build::build_native_module(...)`
  from `build.rs` when generating Rust-native i18n artifacts from a checked-in
  `mf2_i18n.toml` project config

```rust
let output = mf2_i18n::build::build_native_module(
    &mf2_i18n::build::NativeModuleBuildOptions::new(
        "i18n/mf2_i18n.toml",
        std::env::var_os("OUT_DIR").expect("OUT_DIR"),
        "app_i18n",
    ),
)?;
for path in output.rerun_if_changed_paths() {
    println!("cargo:rerun-if-changed={}", path.display());
}
```

The project pipeline writes `id-map.json`, `id-map.sha256`, `.mf2pack` files,
and generated Rust source that defaults to `mf2_i18n::define_i18n_module!`.

## Runtime Paths

`mf2_i18n` supports three public runtime paths:

- filesystem runtime: use `mf2_i18n::Runtime` when the host loads
  `manifest.json`, `id-map.json`, and `.mf2pack` files from disk
- embedded runtime: use `mf2_i18n::EmbeddedRuntime` when the host ships packs
  in-binary
- native bridge: use `mf2_i18n::NativeLocalizer` when the host needs locale
  negotiation and a Rust-owned runtime surface

For std targets, `mf2_i18n::Runtime::format()` and
`mf2_i18n::NativeLocalizer::format()` use `mf2_i18n::StdFormatBackend` by
default. `mf2_i18n::EmbeddedRuntime::format()` still defaults to
`UnsupportedFormatBackend`; use `format_with_backend(...)` when an embedded
host needs locale-sensitive plural, number, date, time, datetime, unit, or
currency output.

`mf2_i18n::NativeLocalizer` keeps strict translation and key-fallback
convenience separate. Use `tr(...)` or `tr_with_args(...)` when the caller
wants a `Result`, and use `tr_or_key(...)` or `tr_with_args_or_key(...)` when
key fallback is intentional.

## Build And Codegen Paths

Use `mf2_i18n::build::build_native_module(...)` for Rust applications that want
one canonical build-script pipeline over a checked-in `mf2_i18n.toml` project
file and `messages.json` locale catalogs.

Use `platform-bundle.json` when a host generates native code or resources
instead of calling the Rust runtime directly. That contract is still exposed
through `mf2_i18n::PlatformBundle` and the namespaced `mf2_i18n::build::*`
surface.

Formatter calls may include literal named options such as
`{ $total :number style=percent minimum-fraction-digits=2 }`. The build
pipeline preserves those options in compiled artifacts and passes them through
to runtime format backends. Unknown formatter names are rejected at build time
instead of compiling as identity formatting.

Datetime arguments use `mf2_i18n::DateTimeValue`, with explicit
`unix_seconds(...)` and `unix_milliseconds(...)` constructors. Runtime
formatting does not infer timestamp units from magnitude.

`mf2_i18n::StdFormatBackend` resolves locale data only from the requested
locale chain. Use `StdFormatBackend::resolution()` to inspect the requested
locale and the resolved plural, number, and date locales.

## Expert Crates

The façade is the canonical consumer path. The lower-level crates remain public
as advanced escape hatches when a project needs tighter control over one layer:

- `mf2_i18n_core`: no_std MF2 execution core and pack model
- `mf2_i18n_runtime`: filesystem runtime internals and expert runtime helpers
- `mf2_i18n_embedded`: embedded runtime internals
- `mf2_i18n_native`: native bridge internals and direct macro crate
- `mf2_i18n_build`: lower-level extraction, validation, compilation, pack, and
  platform bundle modules
- `mf2_i18n_cli`: thin CLI over `mf2_i18n_build`

See [docs/runtime-integration.md](docs/runtime-integration.md) for the runtime decision guide and [docs/platform-consumption.md](docs/platform-consumption.md) for generated-runtime inputs.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE`.
