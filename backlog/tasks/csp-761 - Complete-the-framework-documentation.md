---
id: CSP-761
title: Complete the framework documentation
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-31 04:19'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-738
  - CSP-737
  - CSP-740
  - CSP-743
  - CSP-744
  - CSP-748
  - CSP-747
  - CSP-749
  - CSP-753
  - CSP-756
  - CSP-758
  - CSP-760
  - CSP-732
  - CSP-720
documentation:
  - docs/plans/tui-framework/gap-analysis.md
  - docs/adr/0150-tui-framework-workspace-crate.md
priority: medium
type: docs
ordinal: 847000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The skeleton from the documentation story gives every control a page template, and each extraction story fills in its own page. This story checks that the whole is complete and usable on its own: someone in another repository should be able to learn the framework, add a control, and find the house rules without reading Conspectus.

Scope: Verify and fill gaps: a reference page for every public control and foundation module, a usage guide for building an app on the framework (the `M` and `Cx` parameters, hosting the modal stack, registering theme roles, writing key tables, using the test kit), the house rules as a reference, and a roadmap built from `docs/plans/tui-framework/gap-analysis.md` trimmed to the framework's own terms. Add a doc-sync test, in the style of `tests/tui_columns_doc.rs`, that fails when a public control has no reference page or a page names a control that does not exist.

TUI changes: none.

Tests and checks: `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passes. The doc-sync test fails on a scratch change that adds a public control without a page.

Blockers: Every control story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every public control and foundation module has a reference page, and a doc-sync test fails when one is missing or stale
- [x] #2 A usage guide explains hosting the modal stack, registering theme roles, writing key tables, and using the test kit
- [x] #3 The house rules are published as a reference and the roadmap lists the gap-analysis tiers
- [x] #4 The framework docs make no reference to Conspectus-only files, types, or commands
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Audit every page under crates/tuikit/docs/controls against the code: compile every rust fence as a doctest (a cfg(doctest) module that includes each page), fix the ones that do not build, and fix stale prose (the pager's Ports section, seams that have landed, scrollbar-column caveats, host-specific wording, the planned focus row, crossterm and examples in the architecture guide, host commands in the contribution guide, the README's landing list).
2. Add crates/tuikit/docs/guide.md: depending on the crate, features, M and Cx, the modal stack and AppFrame/Screen, theme roles and glyphs, key tables and the scoped keymap, help and the status bar, feedback sinks, PathSource, the test kit, and the independence rules; every code sample is a doctest.
3. Add crates/tuikit/docs/house-rules.md (the 18 rules with rationale and the pages that implement each) and crates/tuikit/docs/roadmap.md (gap-analysis tiers 1-3 and skip list in framework terms, Tier 1 items marked done or open); point the contribution guide and the pages at the house-rules page.
4. Add crates/tuikit/tests/doc_sync.rs: fails when a public module has no page, a page names a module that does not exist, a page lacks a template section or the header table, a feature-gated module's page does not name its feature, a page is not compiled as a doctest, the module map misses a module, or a relative link or anchor in the docs is broken. Prove each failure on a scratch change, then remove it.
5. Host side: docs/index.md entry and the gap analysis's pointer to the roadmap; operator-review notes.
6. Checks: independence guard, doc-sync, doctests with features off and with richtext, fmt, clippy (workspace all-features and tuikit features off), the isolated suite, rustdoc -D warnings, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Compiled every page's rust blocks as doctests first (a cfg(doctest) module that includes each page): 53 of 56 built; the overlay page's example, the architecture guide's outcome sample, and the contribution guide's lint attributes did not. Fixed those, then audited the prose page by page against the code and the later stories (scrollbar column rule, footer rule, menus under their steps, the tree's cursor gutter).
Doc-sync test proved on a scratch change (pub mod scratch with no page, a page naming tuikit::ages, a missing Seams section, a broken anchor, a renumbered house rule, an uncompiled page): 6 of its 16 tests failed as expected; scratch removed.
Validation: isolated suite 4840 passed, 0 failed, 1 ignored; tuikit doctests 193 (features off) and 201 (richtext); fmt, clippy (workspace all-features, tuikit features off), rustdoc -D warnings, and the whitespace check clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The framework's documentation now stands alone and is held to the code (commits 316e356a, d48fb8ea, 689a04e4, 0f5c9eff).

- **Usage guide** (`crates/tuikit/docs/guide.md`): a small file browser from the dependency line to the test kit: path dependency and features, `M` and `Cx`, `AppFrame` and `Screen`, the modal stack with steps and paste, theme roles, aliases, overrides, and glyph registration, key tables and their checks, the scoped keymap, generated help and the status bar, toasts, the log, and the clipboard `Write` sink, `PathSource`, the test kit (key scripts, the frame driver, goldens, `assert_conventions`), and the independence rules.
- **House rules** (`docs/house-rules.md`): the eighteen rules, each with why it exists, the pages that carry it out, and what checks it; the contribution guide points there instead of keeping a second copy of the table.
- **Roadmap** (`docs/roadmap.md`): the gap analysis's tiers 1 to 3 and skip list in the framework's own terms. Tier 1 done: select all and none, style-aware goldens, bracketed paste, the spinner, availability (show and dim); open: the command palette, theme presets with `NO_COLOR` and color-depth downgrade.
- **Reference pages**: every public module (36, `richtext` and `testkit` included) has a page on the template. Fixed against the code: the pager's "Ports" section (now "Common readers", generic recipes), landed seams presented as open (half pages, arrow marks, disabled rows, rows on a schema, generated help, the split pane, a menu under a dialog), the scrollbar-column rule, host-specific wording, `strip_ansi` missing from the text page, the overlay page's example that did not compile, the architecture guide's partial outcome sample, its planned `focus` module (now a "Focus" section), crossterm and examples in its tables, host commands in the contribution guide, and the README's landing list.
- **Examples cannot rot**: `src/doc_pages.rs` (only under `cfg(doctest)`) includes every page under `docs/`, so all 63 `rust` blocks compile and run as doctests (62 with features off).
- **Doc-sync test** (`tests/doc_sync.rs`, 16 tests): fails on a public module with no page, a page naming a module that is not public or two pages claiming one, a missing header row or template section or `rust` usage example, a feature gate the page does not name, a house rule numbered under another topic, a page with Rust code that is not compiled, a module map that misses a module or page, and a relative link or anchor that does not resolve. Proved on a scratch change: a `pub mod scratch;` with no page, an `age` page naming `tuikit::ages`, a missing `## Seams left open`, a broken anchor, a renumbered rule, and an uncompiled page each failed their test (6 of 16 failed), then the scratch was removed.

Verification: `cargo test -p tuikit --test doc_sync` and `--test independence` pass; page doctests pass with features off and with `richtext`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -D warnings` and `cargo clippy -p tuikit --all-targets -D warnings` (features off); the isolated suite (4840 passed, 0 failed, 1 ignored); `cargo test -p tuikit` with features off (193 doctests) and with `richtext` (201); `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`; `git diff --check`.

**Regressions**: none (documentation and tests only).

**Consistency wins**: none visible (no TUI change).

Judgment calls are in `docs/plans/tui-framework/operator-review.md` (CSP-761 rows): doctests by page include rather than extracted examples, the extra doc-sync checks, dropping the `focus` module, no ports or limits template section, the house rules page as the one copy, the Tier 1 status calls, the raised independence tripwires, and the decisions index left for CSP-762.
<!-- SECTION:FINAL_SUMMARY:END -->
