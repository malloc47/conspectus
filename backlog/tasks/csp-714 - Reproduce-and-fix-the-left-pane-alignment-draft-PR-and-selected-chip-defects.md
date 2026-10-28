---
id: CSP-714
title: 'Reproduce and fix the left-pane alignment, draft-PR, and selected-chip defects'
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/tui-columns.md
priority: medium
type: bug
ordinal: 800000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The inventory found three left-pane inconsistencies by reading (slice 02, section 12; slice 02, section 8). Group-label alignment is computed over the visible rows, but `docs/tui-columns.md` says it covers the whole tree, and glyph-less bucket headers start 2 cells left of their siblings. A closed draft PR can show a yellow `⇄` beside a red `⊗`, because the PR state-to-glyph rule exists in six places across four files and the draft rule has three variants. On a selected row the chip keeps its normal look because REVERSED on REVERSED is a no-op. The column schema migration would otherwise encode whichever variant it meets first.

Scope: Reproduce each with a test or a `--snapshot` frame. Decide each outcome and record it in `docs/tui-columns.md`: the alignment scope follows the document unless the document is wrong; for the draft rule, propose that state beats draft, so a closed draft shows only the closed glyph, and confirm that with the operator in review; for the selected chip, pick one look and document it. Collapse the PR state-to-glyph logic to one function.

TUI changes:
- Consistency wins: labels align across the whole tree as documented; one PR state-to-glyph rule; the selected row's chip reads the same as other chips on that row
- Regressions or behavior changes: draft-PR glyph combinations change in the PRs view; alignment width can change when rows scroll in or out of view if the whole-tree scope is adopted

Tests and checks: The frame examples in `docs/tui-columns.md` are regenerated with `just tui-columns` and reviewed. Tests cover each PR state crossed with draft.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each of the three defects is reproduced and fixed, or recorded as not reproducible
- [ ] #2 The PR state-to-glyph-and-colour mapping lives in one function that every caller uses
- [ ] #3 `docs/tui-columns.md` states the alignment scope and the draft rule the code implements, and the doc-sync tests pass
- [ ] #4 The operator has confirmed the draft-PR rule in review
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
