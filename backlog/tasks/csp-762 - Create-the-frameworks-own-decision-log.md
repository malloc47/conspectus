---
id: CSP-762
title: Create the framework's own decision log
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-31 05:09'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-761
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
  - docs/adr/README.md
priority: medium
type: docs
ordinal: 848000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decisions that govern framework code live in Conspectus's ADR directory, mixed with decisions about graph resolution and discovery. When the framework leaves the repository, its rationale has to go with it. ADRs 0150 to 0152 and older ADRs on the theme, text input, clipboard, scrollbar, snapshot mode, MVU overlay stack, and overlay key tables each explain a framework design choice (the ADR index groups them under Interactive TUI).

Scope: Write a framework-scoped decision log in `crates/tuikit/docs/decisions/`: one short record per framework-owned decision, each stating the decision and its rationale without Conspectus vocabulary, and linking its origin ADR in Conspectus. Mark the origin ADRs in `docs/adr/README.md` as framework-owned. State the policy: until separation the Conspectus ADRs are canonical for the origin, and after separation the framework log is canonical.

TUI changes: none.

Tests and checks: The independence guard's vocabulary scan passes on the decision log. Every framework-owned origin ADR is linked from the log and marked in the index.

Blockers: Complete documentation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `crates/tuikit/docs/decisions/` has an index and one record per framework-owned decision, written without Conspectus vocabulary
- [x] #2 Each record links its origin ADR, and the Conspectus ADR index marks the framework-owned ADRs
- [x] #3 The canonical-source policy before and after separation is written down
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read every host ADR the framework code rests on (the Interactive TUI section, 0150 to 0198, and the older theme, glyph, text-input, clipboard, scrollbar, snapshot, MVU, keymap, log, wrap, and rich-text dependency records) and sort each into framework-owned, framework-owned in part, or host-only.
2. Write crates/tuikit/docs/decisions/: one short record per framework-owned decision (status, decision, rationale, consequences, alternatives, and an Origin section naming the host ADR numbers with neutral titles), plus records for conventions with no record yet (pages as doctests, mandatory documentation, code hygiene, the guard rules, the decision log itself).
3. Rewrite decisions/README.md: grouped index, the record shape, the canonical-source policy before and after separation, and the host records deliberately not included, with reasons.
4. Mark the framework-owned ADRs in docs/adr/README.md (status-cell annotation plus a legend), add the canonical-source policy to ADR 0150's consequences, and a line in docs/design.md.
5. Extend crates/tuikit/tests/doc_sync.rs: index and record files agree (link, title, status, origins), every record has the sections and an Origin, and, when the host repository is present, every origin exists, the host index marks exactly the origins, and every Interactive TUI record is an origin or listed as not included; the host checks skip, with a note, when the crate stands alone. Self-tests on in-memory input.
6. Run the independence guard, doc_sync, fmt, clippy (workspace all-features and tuikit features off), the isolated suite, rustdoc -D warnings, git diff --check; record judgment calls in operator-review.md; commit in slices.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Read every Interactive TUI ADR (0150 to 0198 in full Decision and Alternatives, the older theme, glyph, text-input, clipboard, scrollbar, snapshot, MVU, log, wrap, keymap, and rich-text dependency ADRs) and sorted them: 42 framework-owned, 22 in part, the rest host-only and listed with reasons in the log's index.
Wrote 70 records (0001 to 0070) plus the index; restated alternatives were checked against each ADR's own Alternatives section.
Proved the new doc-sync checks on scratch edits (an unmarked ADR, an origin pointing at a missing ADR, a dropped index row each failed; restored), and the stand-alone skip by pointing HOST_MANIFEST at a missing file (prints the note, passes; restored).
Validation: isolated suite 4848 passed, 0 failed, 1 ignored (4840 before plus 8 new doc_sync tests); tuikit doctests 193 (features off) and 201 (richtext); fmt, clippy (workspace all-features; tuikit all-targets features off), rustdoc -D warnings, git diff --check clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The framework has its own decision log (commits 6393c99c, 205b4327, 048cf724).

- **Records** (`crates/tuikit/docs/decisions/0001` to `0070`): one per framework decision, grouped as the crate and its boundary, foundations, keys and overlays, feedback, the test kit, text entry, choice and command, reading, data display and chrome, and working on the crate. Each has a status, Decision, Rationale, Consequences, Alternatives considered, and an `## Origin` naming its host ADRs by number under neutral titles, written without host vocabulary. New records cover conventions that had none: pages as doctests (0068, no host ADR), mandatory documentation (0067), code hygiene (0069), the guard rules (0003), and the log itself (0070).
- **Index** (`decisions/README.md`): the canonical-source policy (host ADRs canonical before separation, the log after; a decision that also changes the host gets an ADR and a record together), the record shape, the grouped tables, and the host ADRs deliberately left out with reasons.
- **Host side**: `docs/adr/README.md` marks 64 ADRs `framework-owned` (42) or `framework-owned in part` (22) in the Status cell, with a legend; ADR 0150's consequences and `docs/design.md` state the policy.
- **Test** (`crates/tuikit/tests/doc_sync.rs`, 8 new tests): index and records agree (link, number, title, status, origins); every record has its sections and a readable Origin; with the host present, every origin exists in `docs/adr`, the marks equal the restated ADRs both ways, and every Interactive TUI ADR is restated or listed as left out. Without the host (no `../../Cargo.toml` naming the crate) those checks print a note and pass. The vocabulary tripwire rose from 210 to 260.

Verification: scratch breakages (unmarked ADR, missing origin ADR, dropped row) each failed and the stand-alone skip passed with its note; isolated suite 4848 passed, 0 failed, 1 ignored; tuikit doctests 193 and 201 (richtext); fmt, clippy (workspace all-features, tuikit features off), rustdoc -D warnings, git diff --check.

**Regressions**: none (documentation and tests only).

**Consistency wins**: none visible (no TUI change).

Judgment calls are in `docs/plans/tui-framework/operator-review.md` (CSP-762 rows): the ownership split, the record shape, the Status-cell mark, the extra policy rule, the extra test checks, and the raised tripwire.
<!-- SECTION:FINAL_SUMMARY:END -->
