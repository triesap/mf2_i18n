# Contributing

Thanks for your interest in contributing to mf2_i18n.

## Ways to help

- Report bugs and regressions
- Improve documentation and examples
- Add new builders or framework bindings
- Improve tests and packaging polish

## Development setup

This repository is a Rust workspace. Typical tasks:

- `cargo fmt --check`
- `cargo test --workspace --locked`

## Documentation

- keep `README.md` short and crates.io-focused
- put deeper integration details under `docs/`
- keep public terminology simple and consistent across the repo

## Publishing

Publishing is maintainer-only, local, and contract-gated.

Do not publish with ad hoc `cargo publish` commands. A release candidate must pass
formatting, tests, package verification, and crates.io registry checks before any
crate version is published. Hosted publish workflows are not the release
authority for this project.

## Pull request checklist

- Keep changes focused and well-scoped
- Add or update tests when behavior changes
- Keep public docs and package metadata in sync
- Avoid introducing new unsafe code

## Code style

- Use idiomatic Rust
- Prefer small, composable helpers
- Favor clear, explicit APIs over cleverness

## License

By contributing, you agree that your contributions are released under the
project license (MIT OR Apache-2.0).
