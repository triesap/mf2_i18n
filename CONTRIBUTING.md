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

Publishing is driven by the repo-owned script and GitHub Actions workflow:

- local dry run: `./scripts/publish-crates.sh dry-run`
- local publish: `./scripts/publish-crates.sh publish`
- CI publish lane: `.github/workflows/publish-crates.yml`

The workflow is manual-only and publishes crates in dependency order. Use the
`from_crate` input to resume after a partial release or crates.io index lag.
Actual publish runs require the `CARGO_REGISTRY_TOKEN` repository secret.

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
