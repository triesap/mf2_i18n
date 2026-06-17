# mf2_i18n Web Hybrid Target RCLD

Status: complete
Created: 2026-06-17
Completed: 2026-06-17
Owner: mf2_i18n

## Goal

Add a production-grade `hybrid` web target that combines literal static JSON
for simple messages with the full MF2 WASM runtime for dynamic messages.

The target should let JavaScript websites minimize browser runtime work while
keeping one MF2 catalog source of truth, one `build-web` command, one generated
TypeScript API, and deterministic generated artifacts.

## Product Contract

Hybrid mode must be a first-class `[[web.targets]]` mode:

```toml
[[web.targets]]
name = "app"
mode = "hybrid"
framework = "vite"
out_dir = "src/generated/mf2"
locales = ["en", "es"]
namespaces = ["home", "checkout"]
release_id = "app-local"
generated_at = "2026-02-01T00:00:00Z"
stale_policy = "fail"
```

The generated application API should look like one localization surface even
though it internally chooses between static JSON and runtime formatting.

Required generated behavior:

- literal no-argument messages may be served from static JSON
- messages with variables, formatter calls, selects, plurals, dates, numbers,
  or currencies must use runtime packs
- TypeScript must expose one `MessageId`, `ArgsFor<K>`, and `format(...)`
  surface across both paths
- runtime packs should lazy-load only when a dynamic message is formatted
- locale negotiation must be shared across static and runtime paths
- stale output and drift checks must behave like the other web targets

## Current Inputs

The completed web-generator RCLD provides the required foundations:

- `plain-json` target generation
- `runtime` target generation
- generated TypeScript metadata and arg inference
- `mf2_i18n_wasm` runtime APIs with stable JS error codes
- web artifact manifests, staged writes, `--check`, and stale policies
- fixture coverage for plain, runtime, and Docusaurus targets
- Radroots Platform Docusaurus downstream proof

`hybrid` is now a first-class non-Docusaurus web target mode with static JSON,
runtime packs, generated TypeScript loaders, CLI support, fixture coverage, and
documented migration guidance.

## Out Of Scope

- npm publishing
- CommonJS-first packages
- Node-only runtime adapters
- JS-side MF2 interpretation
- browser unit formatting before real unit label data exists
- Docusaurus hybrid mode unless a later Docusaurus-specific need is proven

## Slice Map

### RCLD-H01: Hybrid Target Validation And Classifier

Status: complete
Commit: 73a83e6

Enable `hybrid` validation and add a deterministic classifier that splits
message metadata into static-safe and runtime-required groups.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build web_metadata
cargo test -p mf2_i18n_build config
```

### RCLD-H02: Hybrid Artifact Builder

Status: complete
Commit: f1394d5

Generate the combined artifact set: static JSON, runtime manifests, id maps,
packs, runtime assets, metadata, and `mf2-web-manifest.json`.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build hybrid
cargo test -p mf2_i18n_build web
```

### RCLD-H03: Hybrid TypeScript Loader API

Status: complete
Commit: 853f055

Generate one TypeScript API that routes literal messages to static JSON and
dynamic messages to the WASM runtime while preserving typed message ids and
arguments.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_build hybrid
```

### RCLD-H04: `build-web` CLI Hybrid Enablement

Status: complete
Commit: 83603cd

Wire `mf2_i18n_cli build-web --target <name>` to the hybrid builder, including
`--check`, output override handling, and typed errors.

Green lane:

```sh
cargo fmt --all -- --check
cargo test -p mf2_i18n_cli command_build_web
```

### RCLD-H05: Fixture App Coverage

Status: complete
Commit: c6b3649

Add a fixture project with mixed literal and dynamic messages and assert that
generated outputs and TypeScript loader files are deterministic.

Green lane:

```sh
cargo fmt --all -- --check
cargo test
```

### RCLD-H06: Product Documentation And Migration Guidance

Status: complete
Commit: f5182e7

Update `README.md` and `docs/integration.md` with hybrid target usage,
tradeoffs, output files, and migration guidance from separate plain/runtime
targets.

Green lane:

```sh
cargo fmt --all -- --check
cargo test
```

### RCLD-H07: Final Hybrid Gate

Status: complete
Commit: final closure checkpoint

Run the final validation matrix, reconcile the generated API against the
completed plain/runtime/Docusaurus surfaces, and decide whether any downstream
Radroots Platform fixture should consume hybrid mode.

Green lane:

```sh
cargo fmt --all -- --check
cargo test
```

Downstream decision:

- Docusaurus sites should keep using the Docusaurus-native target because hybrid
  intentionally rejects Docusaurus framework mode.
- JavaScript app consumers with mixed literal and dynamic web copy should adopt
  hybrid as one target instead of parallel `plain-json` and `runtime` targets.
- The next downstream integration that adopts hybrid must run its own
  `build-web --target <name> --check` lane and its website E2E tests against
  the generated hybrid output.
- Final validation also hardened timestamp-only platform test temporary paths
  after the full suite exposed a parallel-test collision.

## Final Validation

Final gate:

```sh
cargo fmt --all -- --check
cargo test
git diff --check
```

## Next Required RCLD Slices

None. The hybrid web target RCLD is complete.
