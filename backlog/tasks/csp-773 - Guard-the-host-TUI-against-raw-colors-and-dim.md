---
id: CSP-773
title: Guard the host TUI against raw colors and dim
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 10:57'
updated_date: '2026-10-30 23:21'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 859000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-721 replaced every raw `Color::` and `Modifier::DIM` in the TUI production code (`src/tui`, `src/viewer`) with theme tokens, so `[tui.theme]` reaches the forms, previews, and viewer. Only the framework crate has a guard (`crates/tuikit/tests/raw_styles.rs`); nothing stops a new host widget from hard-coding a color again, which would silently ignore the operator's theme. The framework guard shares its Rust sanitizer through `crates/tuikit/tests/support/source_scan.rs`; the test kit (CSP-732) is meant to provide such source-scan helpers to the host.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A host test fails on a raw `Color::` or `Modifier::DIM` in non-test code under `src/tui` and `src/viewer`, ignoring comments, string literals, and `#[cfg(test)]` items
- [x] #2 The palette and grammar adapter (`src/tui/theme.rs`) and the ANSI encoder in `src/tui/snapshot.rs` are the only reviewed exceptions, each with a reason, and a stale exception fails
- [x] #3 The scan names its trees through `tests/support/source_tree.rs` and has a planted-violation test for each tree
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Survey `src/tui` and `src/viewer` production code for `Color::`, `Modifier::DIM`, and ratatui's `Stylize` (whose `.red()`/`.dim()` shorthands are the same raw styles): only `src/tui/theme.rs` names raw colors (the default palette of host roles and harness badges); `src/tui/snapshot.rs` now encodes ANSI through `tuikit::testkit::ansi`, so it names none and needs no exception.
2. Add `tests/raw_style_hygiene.rs`: walk `src/tui` and `src/viewer` through `tests/support/source_tree.rs` (tree tripwires) plus a production-file tripwire, blank comments, literals, and `#[cfg(test)]` items with `tuikit::testkit::source::production_code`, skip unit-test files and files under a `*_tests` module directory (`consistency_tests/`), and match needles at a word start.
3. Reviewed exceptions per file and needle with a reason; an exception that matches nothing fails as stale.
4. Self-tests: a planted violation of each needle in every tree (`assert_scan_covers_every_tree`), prose/test code left alone with path:line findings, exception scoping and staleness, the production tripwire, every exception names a needle and a reason.
5. Add the rule's row to the rule-to-crate table in `tests/support/source_tree.rs` and mention the host's scan in the framework contribution guide's raw-style section; record the judgment calls in operator-review.md.
6. Run fmt, both clippy runs, the isolated suite, doc, examples build, diff check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Survey: no raw `Color::`, `Modifier::DIM`, or `Stylize` in src/tui or src/viewer production code outside src/tui/theme.rs (CSP-721 cleaned them), so nothing needed fixing. src/tui/snapshot.rs encodes ANSI through `tuikit::testkit::ansi` (excepted by the framework guard) and names no color, so an exception for it would be stale; theme.rs's `Color::` is the only one.
Verified a real leak fails: a `Color::Red` appended to src/tui/ui/header.rs made `the_tui_draws_only_with_theme_tokens_and_roles` fail with `src/tui/ui/header.rs:786: `Color::` is a raw color...` (reverted).
Checks: fmt; clippy --workspace --all-targets --all-features -D warnings; clippy -p tuikit --all-targets --no-default-features -D warnings; cargo test -p tuikit --no-default-features; full isolated suite 4782 passed, 0 failed, 1 ignored; rustdoc -D warnings; cargo build --examples; git diff --check: all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/raw_style_hygiene.rs`, the host's raw-style guard (commit d05648ea). It walks `src/tui` and `src/viewer` through `tests/support/source_tree.rs` (tree tripwires of 130 and 21 files) and fails on `Color::`, `Modifier::DIM`, or ratatui's `Stylize` import (its `.red()`/`.dim()` shorthands are the same raw styles) in production code: comments, string literals, and `#[cfg(test)]` items are blanked with `tuikit::testkit::source::production_code`, and unit-test files and files under a `*_tests` module directory (`consistency_tests/`) are skipped. A second tripwire requires at least 75 production files read (101 today), so a test-code filter that skipped everything fails.

Reviewed exceptions are per file and per needle with a reason, and a stale one fails. The only one is `Color::` in `src/tui/theme.rs`: the default palette of the roles the host registers (PR states, node kinds, link rows, chips) and of each harness's badge, domain data that `[tui.theme]` overrides. The story also named the ANSI encoder in `src/tui/snapshot.rs`, but that encoding now lives in `tuikit::testkit::ansi` (excepted by the framework guard), so `snapshot.rs` names no color and an exception for it would be stale.

Existing violations: none found (CSP-721 had replaced them), so no production code changed.

Self-tests: a planted violation of each needle in every tree (`assert_scan_covers_every_tree`); realistic leaks reported as `path:line` while theme reads, prose, comments, strings, `MyColor::`, `#[cfg(test)]` modules, `_tests.rs` files, and `consistency_tests/` files are not; an exception excuses one file and one needle and goes stale; test code recognized by path; every exception names a needle and a reason. A `Color::Red` planted in `src/tui/ui/header.rs` made the real scan fail (reverted).

Docs: the rule's row in the rule-to-crate table of `tests/support/source_tree.rs`; a paragraph in the framework contribution guide's raw-style section saying a host runs the same rule with its own exception list; the judgment calls (the `Stylize` needle, no `Modifier::REVERSED` needle, the snapshot exception) in `docs/plans/tui-framework/operator-review.md`.

Checks: fmt; clippy --workspace --all-targets --all-features -D warnings; clippy -p tuikit --all-targets --no-default-features -D warnings; cargo test -p tuikit --no-default-features; the full isolated suite (4782 passed, 0 failed, 1 ignored); rustdoc with -D warnings; cargo build --examples; git diff --check.

Regressions: none (no visible change; test-only).
Consistency wins: none visible; the host's drawing code is now held to the theme as the framework's is.
<!-- SECTION:FINAL_SUMMARY:END -->
