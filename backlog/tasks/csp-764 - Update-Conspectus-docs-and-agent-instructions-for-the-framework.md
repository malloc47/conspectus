---
id: CSP-764
title: Update Conspectus docs and agent instructions for the framework
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:28'
updated_date: '2026-10-31 07:04'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-761
documentation:
  - CLAUDE.md
  - docs/adr/README.md
priority: medium
type: docs
ordinal: 850000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`CLAUDE.md` and `AGENTS.md` carry TUI conventions that now describe framework controls: a new view, menu, or key reuses the existing action, menu, and lifecycle code; `docs/tui-columns.md` is the living reference for the left panel; the consistency tests assert behavior across views; and Pre-Main Invariants name the commands. Operator-facing changes (theme key aliases, removed view values, right-pane keys, new close and confirm behavior) need to reach the operator guide.

Scope: Update `CLAUDE.md` and `AGENTS.md` so the TUI conventions name the framework's controls, key tables, test kit, and house rules, and point to where the consistency tests live now. Update `docs/operations.md` for operator-visible changes and `CHANGELOG.md`. Add the amendments to the ADR index for ADRs 0030, 0031, 0052, 0076, 0085, 0097, and 0123 as their stories landed. Mark the architecture review's findings F4, F5, and F7 as resolved or partly resolved.

TUI changes: none; this documents changes made by other stories.

Tests and checks: `git diff --check` and the doc-sync tests pass. A reader following `CLAUDE.md` to add a menu is sent to the framework's `ActionMenu` page.

Blockers: Complete documentation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `CLAUDE.md` and `AGENTS.md` describe the framework-based TUI conventions and the workspace commands
- [x] #2 `docs/operations.md` and `CHANGELOG.md` list every operator-visible change from the milestone
- [x] #3 The ADR index records the amendments the milestone made to earlier ADRs
- [x] #4 `docs/tui-architecture-review.md` marks the findings the milestone resolved
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. AGENTS.md (CLAUDE.md is its symlink): keep the TUI-conventions bullet and add one that sends a new view, menu, or key to the framework's controls, key tables, house rules, guide, and control pages (a menu to controls/action_menu.md) and names both halves of the consistency tests (src/tui/consistency_tests and the conformance suite's assert_conventions); one for the workspace (crates/tuikit, independence and vocabulary rules, the guard tests, just tuikit-features, just separation-rehearsal --self-test); one under Decision Records for framework records beside an ADR. Pre-Main Invariants take the bullets CSP-785 and CSP-763 proposed. Leave every other rule and the Backlog.md block alone.
2. docs/adr/README.md: name every amendment the milestone made to an earlier ADR (0024, 0031, 0052, 0064, 0097, 0114, 0177) and the refinements stories recorded without a new ADR; add forward links in the Status sections of 0024, 0052, and 0064. In a separate commit, fix the older index rows whose cell omits an amendment the ADR records.
3. docs/tui-architecture-review.md: a table marking each finding's status (F4, F5, F7 resolved by the milestone; F1, F2, F3, F6 fixed earlier, verified against the code).
4. CHANGELOG.md: correct bullets a later story overtook and add the operator-visible changes no bullet named, from the final summaries and operator-review.md.
5. docs/operations.md: a grouped "What changed in the TUI" section with the [tui.theme] migration notes, pointing at the sections that document each.
6. docs/plans/tui-framework/README.md: status, a Status column, a follow-ups table, links to operator-review.md and execution-brief.md; docs/index.md entries.
7. Stale references in design.md and README.md; leave historical ADR bodies, audits, and plan artifacts alone.
8. Record judgment calls in operator-review.md; run fmt, both clippy runs, the isolated suite, rustdoc, just tuikit-features, just separation-rehearsal --self-test, git diff --check; commit in slices; close the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in eight docs commits: AGENTS.md (framework conventions, workspace, decision records, two Pre-Main Invariants); the ADR index (milestone amendments, refinements, and forward links in 0024/0052/0064), then 22 older index omissions separately; the architecture review's findings table; operations.md "What changed in the TUI" with the theme migration notes and the CHANGELOG corrections and additions; the plan README status and follow-ups table and docs/index.md; stale references in design.md and README.md; operator-review rows. Doc-sync (`cargo test -p tuikit --test doc_sync`) passed after the index edits.

Checks: cargo fmt --all -- --check; cargo clippy --workspace --all-targets --all-features -- -D warnings; cargo clippy -p tuikit --all-targets -- -D warnings; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps; just tuikit-features; just separation-rehearsal --self-test (4m 35s, both leaks caught, scratch removed); git diff --check: all pass. Isolated full suite 4849 passed, 0 failed, 1 ignored (the theme-docs, doc-sync, and search-docs tests read the edited files). An ADR index check over every Status section finds no omitted amendment (one false positive, 0194's "as amended by 0176").
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Brought Conspectus's own docs and agent instructions up to the framework milestone, in nine docs commits.

- `AGENTS.md` (`CLAUDE.md` stays its symlink): a new Implementation Notes bullet sends a new view, menu, or key to the framework's controls, key tables, house rules, guide, and control pages (a menu to `crates/tuikit/docs/controls/action_menu.md`) and names both halves of the consistency tests (`src/tui/consistency_tests/` and the conformance suite's `assert_conventions`); another describes the workspace (`crates/tuikit`, `--workspace`, the independence and vocabulary rules with `DOMAIN_TERMS`, the guard tests, `just tuikit-features`, `just separation-rehearsal --self-test`); a Decision Records bullet asks for a framework record beside the ADR. The capabilities and text-rule notes say the framework is covered. The Pre-Main Invariants take CSP-785's and CSP-763's proposed bullets word for word. Every other rule and the Backlog.md block are unchanged.
- `docs/adr/README.md`: the milestone's amendments of 0024, 0031, 0052, 0064, 0097, 0114, and 0177, and the refinements stories recorded without a new ADR (0071, 0155, 0156, 0175, 0181, 0190, 0191); forward links in the Status sections of 0024, 0052, and 0064. A separate commit fixes 22 older rows whose cell omitted an amendment the ADR records.
- `docs/tui-architecture-review.md`: a table of where each finding stands, checked against the code (F4, F5, F7 resolved by the milestone, F1's residue closed; F1, F2, F3, F6 fixed earlier).
- `docs/operations.md`: "What changed in the TUI", by area (views and the command line, main-screen keys, the right pane, dialogs and text fields, the look, messages and hints, forms and pins), and "Moving a `[tui.theme]` across". `CHANGELOG.md`: three bullets a later story overtook corrected; the changes no bullet named added (state-file view, left-pane `e`, theme precedence, `link_id`, info color, Shift in fields, find chip, empty value, mux chips, unseen chip, right-pane refusal, and more); a bullet for the framework crate.
- `docs/plans/tui-framework/README.md`: executed status, a Status column, a follow-ups table, links to the execution brief and operator review; `docs/index.md` entries. Stale references fixed in `docs/design.md` and `README.md` (including three ADR links broken since m-23).

Verified with: cargo fmt --check; clippy --workspace --all-targets --all-features -D warnings; clippy -p tuikit --all-targets -D warnings; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps; `just tuikit-features`; `just separation-rehearsal --self-test` (passed, both leaks caught); `git diff --check`; the isolated suite, 4849 passed, 0 failed, 1 ignored. Judgment calls are the CSP-764 rows of operator-review.md.

**Regressions**: none (documentation only; no visible TUI change).

**Consistency wins**: none visible.
<!-- SECTION:FINAL_SUMMARY:END -->
