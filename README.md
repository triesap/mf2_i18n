# mf2_i18n

`mf2_i18n` is the main Rust crate for Unicode MessageFormat v2 (MF2).

It provides:

- a `no_std` MF2 core
- build-time compilation from `mf2_i18n.toml`
- checked-in `id_salt.txt` project inputs
- portable `.mf2pack` artifacts instead of runtime parsing
- runtime surfaces for filesystem, embedded, and native hosts

## Install

Most applications should start with `mf2_i18n` and enable only the features
they need.

```toml
[dependencies]
mf2_i18n = { version = "0.1.0", features = ["native"] }

[build-dependencies]
mf2_i18n = { version = "0.1.0", features = ["build"] }
```

## Feature Map

- no features: core MF2 types, values, and locale negotiation helpers
- `runtime`: load `manifest.json`, `id-map.json`, and `.mf2pack` files from
  disk with `mf2_i18n::Runtime`
- `embedded`: ship id maps and packs in-binary with
  `mf2_i18n::EmbeddedRuntime`
- `native`: use `mf2_i18n::NativeLocalizer` and
  `mf2_i18n::define_i18n_module!` for Rust-owned application localization
- `build`: compile a checked-in `mf2_i18n.toml` project from `build.rs`
- `std_backend`: expose `mf2_i18n::StdFormatBackend` directly

On std targets, `Runtime` and `NativeLocalizer` use `StdFormatBackend` by
default. `EmbeddedRuntime` requires `format_with_backend(...)` for
locale-sensitive formatting.

## Minimal `build.rs`

```rust
fn main() {
    let output = mf2_i18n::build::build_native_module(
        &mf2_i18n::build::NativeModuleBuildOptions::new(
            "i18n/mf2_i18n.toml",
            std::env::var_os("OUT_DIR").expect("OUT_DIR"),
            "app_i18n",
        ),
    )
    .expect("build i18n module");

    for path in output.rerun_if_changed_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
```

This generates:

- `id-map.json`
- `id-map.sha256`
- `*.mf2pack`
- generated Rust source that defaults to `mf2_i18n::define_i18n_module!`

## Main Entry Points

- `mf2_i18n::Runtime`
- `mf2_i18n::EmbeddedRuntime`
- `mf2_i18n::NativeLocalizer`
- `mf2_i18n::build::build_native_module(...)`

## Lower-Level Crates

`mf2_i18n` is the default entry point. If you need tighter control over one
layer, the workspace also exposes `mf2_i18n_core`, `mf2_i18n_runtime`,
`mf2_i18n_embedded`, `mf2_i18n_native`, `mf2_i18n_build`, and
`mf2_i18n_cli`.

## More Docs

- [integration.md](docs/integration.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)

## License

MIT OR Apache-2.0. See [LICENSE](LICENSE).
