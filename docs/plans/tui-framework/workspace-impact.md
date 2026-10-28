# Workspace Conversion: What It Touches

Findings from reading the repo at `ca668a3`. They feed the first stories of
the milestone (workspace conversion, guardrails). Nothing here has been
changed yet.

## Build And CI

- `Cargo.toml` is a single `[package]` with a root `[lints.clippy]` table, a
  `[features] snapshot` flag (dev-only `tui --snapshot`), and `[dev-dependencies]`
  that include `tui-pantry`. Converting to a workspace means a `[workspace]`
  table, `[workspace.dependencies]` for shared versions, and
  `[workspace.lints]` so both crates inherit the curated clippy levels
  (`lints.workspace = true` in each member).
- `cargo fmt --all`, `cargo clippy --all-targets --all-features`,
  `cargo test --all-targets --all-features`, and `cargo nextest run ...` in
  `justfile`, `.github/workflows/ci.yml`, `flake.nix`'s shell banner, and the
  Pre-Main Invariants in `CLAUDE.md`/`AGENTS.md` are package-scoped. With the
  root still a package, they cover only that package unless they gain
  `--workspace` (or the root sets `default-members`). Missing this would let
  the framework crate's tests go unrun in CI without any failure.
- `cargo doc --no-deps` with `RUSTDOCFLAGS=-D warnings` also needs
  `--workspace`.
- `flake.nix` only defines a dev shell (no package build), so the Nix impact
  is the banner text. `rust-toolchain.toml` and `Cargo.lock` stay shared at
  the workspace root.

## Source-Scanning Tests That Would Go Vacuous

These walk fixed paths under `src/` and would silently stop covering code that
moves into the new crate (they would still pass):

| Test | What it scans |
| --- | --- |
| `tests/test_module_hygiene.rs` | every `.rs` file for inline `mod tests` over 100 lines |
| `tests/comment_hygiene.rs` | comments for backlog IDs |
| `tests/overlay_keymap_hygiene.rs` | `src/tui/widgets/**` and `src/tui/runtime/overlay_keys.rs` for hand-matched `KeyCode` and hand-written key hints |
| `tests/capabilities_static.rs` | source for process, socket, signal, and write paths against `docs/capabilities.toml` |
| `src/tui/consistency_tests/text.rs` | string literals and rendered screens for ADR/backlog/doc-path leaks |
| `src/tui/consistency_tests/docs.rs` | reads `docs/tui-columns.md` via `CARGO_MANIFEST_DIR` (changes if the tests move) |

Each needs to scan the framework crate's tree too (or move with the code it
guards and gain a "scanned at least N files" tripwire so a bad path fails
instead of passing empty).

## What The Framework Must Not Own

- Process spawning: `exec::command` is used by `messages.rs`, `resume.rs`,
  `effect.rs`, `runtime.rs`, `runtime/launch.rs`, `runtime/visibility.rs`,
  `preview/process.rs`, and `preview/git.rs`. None of those is a widget. The
  capability tests (`docs/capabilities.toml`) are keyed to conspectus's
  `exec::command`; the framework crate should spawn nothing, and a guard
  should fail the build if it ever does.
- `clipboard.rs` is already domain-free (OSC 52 with a hand-rolled base64
  encoder, no subprocess), so it is a framework candidate as-is.

## Test Assets That Move With Code

- `insta` snapshots live in `src/tui/snapshots/` (19 files) and
  `src/tui/widgets/snapshots/` (18 files); they follow the tests that own them,
  and `insta` resolves the directory relative to the test file.
- `examples/pantry.rs` (the `tui-pantry` harness) imports
  `conspectus::tui::widgets::*`; framework controls should be previewable
  from the framework crate's own examples without the conspectus domain.
- Integration tests that import `conspectus::tui`: `tests/testing_replay.rs`,
  `tests/tui_tmux_e2e.rs`, `tests/support/replay.rs`.

## Public-API Surface Today

`lib.rs` exposes `tui` as `#[doc(hidden)] pub mod tui` because the binary,
integration tests, and the pantry example reach in. A framework crate needs a
deliberate public API; `pub(crate)` items that widgets currently share across
modules become either framework-public or stay behind in conspectus.
