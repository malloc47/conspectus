# Phase 00: Project Foundation

## Summary

Create the Rust project baseline for Conspectus without implementing graph
discovery behavior yet. The result should be a buildable, testable,
library-first CLI skeleton that future phases can extend.

## End-State Behavior

- `conspectus --help` and `conspectus --version` work.
- The crate has a thin CLI entrypoint and a library entrypoint.
- Local check commands are discoverable and match ADR 0007.
- The project remains non-mutating except for normal build and test artifacts.

## Implementation Changes

- Add Rust project files: `Cargo.toml`, `rust-toolchain.toml`, `src/lib.rs`,
  and `src/main.rs`.
- Add a `justfile` with check targets for formatting, linting, tests, nextest,
  and whitespace diff checks.
- Add initial dependencies: `clap`, `serde`, `serde_json`, `toml`,
  `toml_edit`, `indexmap`, `anyhow`, and `thiserror`.
- Add test dependencies: `assert_cmd`, `predicates`, `tempfile`, `insta`,
  `rstest`, and `proptest`.
- Keep the module layout aligned with ADR 0007: `model`, `resolve`,
  `discovery`, and `output` can begin as empty or minimal modules.

## Tests

- CLI smoke test: `conspectus --help` exits successfully and includes the
  command name.
- CLI smoke test: `conspectus --version` exits successfully.
- Check target test: `just check` runs the complete baseline check suite.

## Manual Checks

```sh
nix develop
just check
cargo run -- --help
```

## Assumptions

- CI wiring can be added in a follow-up once the local check commands exist.
- No graph schema is committed in this phase beyond placeholder module
  boundaries.
