# Contributing

Thanks for your interest in contributing to mf2-i18n.

## Ways to help

- Report bugs and regressions
- Improve documentation and examples
- Add new headless builders or framework bindings
- Expand accessibility and keyboard coverage

## Development setup

This repository is a Rust workspace. Typical tasks:

- `cargo fmt`
- `cargo test`

## Pull request checklist

- Keep changes focused and well-scoped
- Add or update tests when behavior changes
- Keep public APIs documented
- Avoid introducing new unsafe code

## Code style

- Use idiomatic Rust
- Prefer small, composable helpers
- Favor clear, explicit APIs over cleverness

## Accessibility

All components should follow WAI-ARIA APG patterns where applicable.
If behavior changes affect keyboard interaction or focus, include tests.

## License

By contributing, you agree that your contributions are released under the
project license (MIT OR Apache-2.0).
