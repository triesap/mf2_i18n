# mf2_i18n

`mf2_i18n` is the main Rust crate for Unicode MessageFormat v2 (MF2).

It provides:

- a `no_std` MF2 core
- build-time compilation from `mf2_i18n.toml`
- checked-in `id_salt.txt` project inputs
- portable `.mf2pack` artifacts instead of runtime parsing
- runtime surfaces for filesystem, embedded, and native hosts
- generated web artifacts for JavaScript apps, React-style bundlers, and
  Docusaurus sites
- browser WASM bindings for JavaScript apps that need full MF2 behavior

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

- `manifest.json`
- `id-map.json`
- `id-map.sha256`
- `packs/*.mf2pack`
- `platform-bundle.json`
- generated Rust source that defaults to `mf2_i18n::define_i18n_module!`

## Main Entry Points

- `mf2_i18n::Runtime`
- `mf2_i18n::EmbeddedRuntime`
- `mf2_i18n::NativeLocalizer`
- `mf2_i18n::build::build_native_module(...)`
- `mf2_i18n::build::build_project_runtime_artifacts(...)`

## Web And JavaScript

Websites declare generated outputs in `mf2_i18n.toml` with named web targets:

```toml
default_locale = "en"
source_dirs = ["locales"]
project_salt_path = "id_salt.txt"

[[web.targets]]
name = "app"
mode = "runtime"
framework = "vite"
out_dir = "src/generated/mf2"
locales = ["en", "es"]
namespaces = ["home", "checkout"]
release_id = "app-local"
generated_at = "2026-02-01T00:00:00Z"
stale_policy = "fail"
```

JavaScript projects should invoke the shared launcher from package scripts:

```json
{
  "devDependencies": {
    "@mf2-i18n/cli": "0.1.0"
  },
  "scripts": {
    "mf2:gen": "mf2-i18n build-web --config i18n/mf2_i18n.toml --target app",
    "mf2:check": "mf2-i18n check-web --config i18n/mf2_i18n.toml --target app"
  }
}
```

Run:

```sh
npm run mf2:gen
npm run mf2:check
```

`@mf2-i18n/cli` is a Node launcher for the Rust `mf2_i18n_cli` binary. It
resolves a configured binary, environment-provided binary, package-local native
binary, Cargo manifest, or adjacent source checkout, then delegates without
shell expansion. Rust workspaces may still call
`cargo run -p mf2_i18n_cli -- build-web --config i18n/mf2_i18n.toml --target app`
directly when developing the crates.

Supported target modes:

- `plain-json`: literal-only JSON for JavaScript apps that do not run MF2 in
  the browser
- `runtime`: full MF2 runtime artifacts, packs, TypeScript metadata, and
  generated browser loader helpers
- `hybrid`: literal messages as static JSON plus lazy MF2 runtime packs for
  variables, formatters, selects, plurals, dates, numbers, and currencies
- `docusaurus`: Docusaurus-native `i18n/<locale>/code.json`, theme
  translation JSON, docs plugin metadata, localized MDX, and generated React
  page message metadata

Plain JSON mode rejects variables, formatters, select, and plural messages.
Runtime mode supports MF2 variables, formatters, selects, plurals, dates,
numbers, and currencies through generated loaders and `mf2_i18n_wasm`. Hybrid
mode exposes one generated TypeScript API while routing literal messages to
static JSON and loading runtime packs only for dynamic messages.

For Docusaurus:

```toml
[[web.targets]]
name = "docs"
mode = "docusaurus"
framework = "docusaurus"
out_dir = "src/generated/mf2"
docs_source_dir = "docs"
docusaurus_i18n_dir = "i18n"
docusaurus_sidebar_id = "docsSidebar"
locales = ["en", "es"]
namespaces = ["home", "docs", "docusaurus"]
stale_policy = "fail"
```

The generated Docusaurus files intentionally use Docusaurus locale routing and
translation conventions. Apps should remove custom sync scripts once
`build-web --target docs` owns those outputs.

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
