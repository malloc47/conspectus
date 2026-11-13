---
id: CSP-785
title: Check the framework with its cargo features off in CI and `just check`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 22:32'
updated_date: '2026-10-30 23:38'
labels:
  - tui-framework
milestone: m-25
dependencies: []
ordinal: 871000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The framework crate has features a host turns on (`richtext`, which pulls in the rich-text crates, and `testkit`), and ADR 0152 and ADR 0181 promise that `tuikit` builds and passes its tests with them off, so a program that shows no rich content pays for none of the parsing crates. CI and `just check` run only `--all-features`, and the workspace's feature unification turns `richtext` on whenever the host is built, so a change that makes code outside `tuikit::richtext` need the feature (or leaves an item only that module uses ungated, which warns without it) passes every check today and is found only by hand. CSP-749 ran the feature-off build manually (`cargo clippy -p tuikit --all-targets -- -D warnings`, `cargo test -p tuikit --no-default-features`).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CI and `just check` build, lint (warnings denied), and test `tuikit` with its features off, and with `richtext` alone
- [x] #2 Code outside `tuikit::richtext` that names one of its items, or an item only that module uses left ungated, makes the check fail
- [x] #3 The framework contribution guide and `docs/plans/tui-framework/workspace-impact.md` name the check
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a `tuikit-features` recipe to the justfile and to `check`: clippy (-D warnings) of `tuikit` alone with `--no-default-features`, lib only (so `testkit` is off too) and `--all-targets`, then the same with `--features richtext`; then `cargo test -p tuikit --no-default-features` and with `--features richtext`. `-p tuikit` keeps the host's features out (resolver 3), so these builds see the framework's own feature sets.
2. Mirror the commands as CI steps in `.github/workflows/ci.yml`, and name the recipe in the flake banner.
3. Verify locally: every command passes; a plant outside `tuikit::richtext` that names one of its items, and an item only that module uses with its gate removed (`glyphs::TABLE_BORDERS`), each fail the check (reverted).
4. Name the check in the contribution guide (Checks before you finish) and `docs/plans/tui-framework/workspace-impact.md`. CLAUDE.md/AGENTS.md Pre-Main Invariants: an agent instruction cannot authorize editing the agent instruction file, so propose the bullet in operator-review.md for the operator to add.
5. Run fmt, both clippy runs, the isolated suite, doc, examples build, diff check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
All six commands pass locally (`just tuikit-features`, exit 0): 1132 lib tests without `richtext`, 1171 with; doctests 131 and 138.
AC #2 evidence (both reverted): (a) `pub fn planted_leak(..) { crate::richtext::ansi_text(source, false) }` appended to crates/tuikit/src/text.rs: both features-off clippy runs fail with E0433 "cannot find `richtext` in the crate root"; the richtext-alone runs and the workspace `--all-features` clippy pass, so only the new check catches it. (b) the `#[cfg(feature = "richtext")]` gate removed from `glyphs::TABLE_BORDERS`: both features-off runs fail with "constant `TABLE_BORDERS` is never used" (-D warnings); the workspace clippy passes.
CI YAML parses (yq lists the two new steps); `just --dry-run tuikit-features` lists the six commands; the dev shell banner shows the recipe.
CLAUDE.md/AGENTS.md not edited: changing the agent instruction file needs the operator; the proposed invariant bullet is in operator-review.md.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The framework is now checked with its cargo features off and with `richtext` alone (commit f03321fe). A new `just tuikit-features` recipe, part of `just check`, and two CI steps ("Lint/Test the framework with its features off and with richtext alone") run:

- `cargo clippy -p tuikit --no-default-features -- -D warnings`, lib-only and with `--all-targets`;
- the same two with `--features richtext`;
- `cargo test -p tuikit --no-default-features`, with and without `--features richtext`.

`-p tuikit` keeps the host's features out (resolver 3), so the builds see the framework's own feature sets. The lib-only lints also build without `testkit`, which the crate's own tests always turn on through its self dev-dependency.

Verification: every command passes locally (1132 lib tests without `richtext`, 1171 with). Two plants, both reverted, show what the check catches. Code in `text.rs` that calls `crate::richtext::ansi_text` fails the features-off runs with E0433. Removing the `richtext` gate from `glyphs::TABLE_BORDERS` fails them with "constant `TABLE_BORDERS` is never used". In both cases the workspace `--all-features` clippy passes, so before this story nothing caught either. The CI YAML parses (yq), and `just --dry-run tuikit-features` and the dev shell banner show the recipe.

Docs: the contribution guide's "Checks before you finish" explains why the workspace checks miss this and lists the six commands; `docs/plans/tui-framework/workspace-impact.md` names the check; the flake banner and the README's `just check` comment mention it. No ADR: the check enforces what ADR 0152 and ADR 0181 already promise.

Not done, for the operator: the Pre-Main Invariants in `CLAUDE.md`/`AGENTS.md` were not edited, because the implementing agent may not change its own instruction file on another agent's request. The proposed bullet is in `docs/plans/tui-framework/operator-review.md`.

Checks: fmt; clippy --workspace --all-targets --all-features -D warnings; `just tuikit-features`; the full isolated suite (4782 passed, 0 failed, 1 ignored); rustdoc -D warnings; cargo build --examples; git diff --check.

Regressions: none (no visible change; build and CI only).
Consistency wins: none visible.
<!-- SECTION:FINAL_SUMMARY:END -->
