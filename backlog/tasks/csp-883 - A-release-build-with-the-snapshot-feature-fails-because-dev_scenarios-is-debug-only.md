---
id: CSP-883
title: >-
  A release build with the snapshot feature fails because dev_scenarios is
  debug-only
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14 22:31'
updated_date: '2026-11-15 00:20'
labels:
  - tui
dependencies: []
priority: low
type: bug
ordinal: 960000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the scroll-stall work (CSP-874): `cargo build --release --features snapshot` fails on main because the `dev_scenarios` module is compiled only in debug builds, while the snapshot feature (the way CLAUDE.md says to render frames with `conspectus tui --snapshot`) refers to it. Timing measurements need a release build, so that work had to build the release binary without the feature. Reproduce with `scripts/dev-env.sh cargo build --release --features snapshot`. Decide whether the snapshot feature should work in release builds (compile `dev_scenarios` whenever the feature is on) or whether the docs should say it is debug-only. Not verified by the coordinator; reported by the implementing agent.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 cargo build --release --features snapshot compiles, and the release binary renders a frame of tests/fixtures/showcase.json with conspectus tui --snapshot; a release build without the feature still has no --snapshot flag and no dev_scenarios
- [x] #2 CI's Rust job checks the release build with and without the snapshot feature
- [x] #3 ADR 0067 records the choice in an amendment, and the feature's docs no longer imply it is debug-only
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduced: `cargo check --release -p conspectus --features snapshot` fails with five errors, all `crate::dev_scenarios` (the recorded panes and PR views snapshot mode reads) missing because `src/lib.rs` compiles the module only for `any(test, debug_assertions)`.
2. Decide: the feature works in release. Gate `dev_scenarios` on `any(test, debug_assertions, feature = "snapshot")`, as the snapshot code that calls it already is. A release build without the feature still carries neither; `conspectus dev` stays debug-only.
3. Cover it: a step in CI's Rust job checks the release build with and without the feature (every other step has debug assertions on); validate the YAML.
4. Prove it: `cargo build --release --features snapshot`, then render a frame of the showcase fixture with the release binary; confirm a release build without the feature has no --snapshot flag.
5. Record the choice as an amendment to ADR 0067; update the module doc, the Cargo.toml feature comment, CLAUDE.md's snapshot note if it says debug-only, and CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Chose to make the feature work in release: `dev_scenarios` is gated on `any(test, debug_assertions, feature = "snapshot")`, as the snapshot code that calls it already was. A release+snapshot binary therefore compiles the scenario builder too, with no command to run it (`conspectus dev` stays debug-only); a featureless release build carries neither (checked: no `dev` subcommand, no `--snapshot` flag, no scenario strings in the binary).
Verified: reproduced 5 errors with `cargo check --release -p conspectus --features snapshot` before; after, `cargo build --release -p conspectus --features snapshot` succeeds and `target/release/conspectus tui --snapshot --snapshot-fixture tests/fixtures/showcase.json --snapshot-fixture-root <world> --snapshot-now 1790769600 --snapshot-keys 2` renders the mux view byte-identical to the debug build's frame; `cargo check --release -p conspectus` and `cargo clippy --release -p conspectus --features snapshot -D warnings` pass; CI YAML parsed with the `yaml` package and passed actionlint (via nix); just check: 5801 passed, 1 skipped. Filed CSP-885: `tui --help` names `--snapshot` in builds without the feature (pre-existing).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`cargo build --release --features snapshot` now compiles: `dev_scenarios` (whose recorded panes and PR views snapshot mode reads) builds whenever the feature is on, not only in tests and debug builds. A release build without the feature still has no snapshot code, no scenario builder, and no `conspectus dev`. CI's Rust job gains a step checking the release build with and without the feature. ADR 0067 carries an amendment recording the choice; README, docs/dev-scenarios.md, the Cargo.toml feature comment, the capability manifest's `dev` note, and CHANGELOG say the feature builds in release. Proved by building the release binary with the feature and rendering a showcase frame identical to the debug build's; `just check` passed (5801 passed, 1 skipped). Follow-up filed: CSP-885.
<!-- SECTION:FINAL_SUMMARY:END -->
