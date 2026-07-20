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

## Leptos render modes

The Leptos integration is a shared-library surface and does not select an
application delivery mode. Final applications select CSR, hydration, or SSR
through their Leptos dependency graph.

The compatibility contract covers both pure localized document projection and
live browser effects. SSR consumers must be able to derive document title and
HTML language without a browser global; CSR and hydration consumers may apply
that same projection to the live document. Consumer fixtures qualify native
SSR rendering and the corresponding browser hydration build in addition to
ordinary Trunk CSR use.

The workspace pins the exact Leptos prerelease and commits its root
`Cargo.lock`. This is required because the upstream prerelease crates use
compatible ranges for sibling crates that can otherwise admit an incompatible
beta graph. Applications must also commit and use their own qualified lockfile.

## Status

This crate is pre-1.0. Public APIs are intended to be usable, but may still change
between minor versions.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.
