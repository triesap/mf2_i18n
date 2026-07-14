# mf2_i18n

Unicode MessageFormat 2 tooling and runtime support for Rust.

`mf2_i18n` provides a small Rust facade over build-time catalog tooling, generated
runtime assets, native formatting helpers, embedded packs, and Leptos integration.

## Install

```toml
[dependencies]
mf2_i18n = { version = "0.1.0", default-features = false, features = ["runtime", "native"] }
```

## Features

- `build`: project config, catalog validation, and artifact generation.
- `runtime`: manifest, id map, and runtime loading APIs.
- `embedded`: embedded pack loading.
- `std_backend`: locale-aware standard formatting backend.
- `native`: native runtime helpers.
- `leptos`: Leptos context and rendering helpers.
- `dto-bindgen`: generated DTO metadata for supported public types.

Use the root `mf2_i18n` crate for normal integrations. The support crates are
published so Cargo can resolve the feature graph and so advanced users can depend
on focused lower-level APIs when needed.

## Status

This crate is pre-1.0. Public APIs are intended to be usable, but may still change
between minor versions.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.
