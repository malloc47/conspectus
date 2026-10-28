---
id: CSP-735
title: Extract `PathField`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-733
  - CSP-715
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
  - docs/adr/0097-launch-spec-form-primitive.md
priority: medium
type: enhancement
ordinal: 821000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The tab-completed path widget (`path_omnibox.rs`) imports nothing from the app but its key table, yet it is not behaviorally neutral. It suggests files as well as directories and validates with `exists`, so a file passes for a `cwd`. It reads the filesystem and `$HOME` every frame (one `stat` per path row, a `read_dir` while focused) with no injection point. `~/x` validates but is committed unexpanded. `selected`, `MAX_CANDIDATES`, `source`, `exists`, and `expanded_value()` are dead or unrendered. Two renderers differ on the Tab hint (a 32-cell inline hint versus a mark only). Three `~` expansions exist (inventory slice 03, section 2.2).

Scope: Create `PathField` on `LineEdit` with a `PathSource` trait (listing, stat, home directory, so tests need no real filesystem or `$HOME`), a `PathKind` (directory or file), ranked completion with known candidates ahead of filesystem matches, cached results so rendering does no I/O, and `~` expansion on commit through `expanded_value()`. Remove the dead surface or adopt it. Settle one Tab hint rule. Leave the seam for the ranked candidate list that CSP-455 asks for and for the browse mode of CSP-434.

TUI changes:
- Consistency wins: a file no longer validates as a working directory; `~` paths work; no per-frame disk I/O; one Tab hint rule
- Regressions or behavior changes: the inline Tab hint no longer reserves 32 cells in forms that used it, or appears in forms that did not, depending on the rule chosen

Tests and checks: Tests against an in-memory `PathSource` for ranking, completion, kind, tilde, and caching. The path behaviors from the text-entry defect task pass unchanged.

Blockers: `LineEdit` and the text-entry defect task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `PathField` takes a `PathSource` and a `PathKind`, caches results, and does no I/O while rendering
- [ ] #2 A file does not validate where a directory is required
- [ ] #3 `~` is expanded on commit, and the three `~` implementations are one
- [ ] #4 The dead surface (`selected`, `MAX_CANDIDATES`, `source`, `exists`) is removed or used, and `expanded_value()` has a caller
- [ ] #5 One Tab hint rule applies to every form that has a path field
- [ ] #6 The seam for a ranked candidate list is documented against CSP-455
- [ ] #7 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #8 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #9 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
