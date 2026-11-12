---
id: CSP-818
title: Fix the hook attribution spool defects the live-state research found
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
documentation:
  - docs/plans/live-state/research/06-internal-audit.md
priority: medium
type: bug
ordinal: 904000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The research found defects in the attribution path that live state does not need fixed to work, but that are wrong on their own: the spool key ignores the pane so split-pane agents overwrite each other's attribution, `ACTIVE_TTL_SECONDS` is dead code while the docs say stale records are ignored, an ingest leaves two active links for one pane until the next cycle, and the traced capability test does not cover `hook write`.

Scope: Key the spool by pane (falling back to pid, then cwd); apply the stated TTL or correct the docs and delete the constant, whichever the code supports after a decision recorded in an ADR note; supersede older hook links for the same pane during ingest; update ADR 0028 and `docs/operations.md`, which still describe `hooks.sqlite3` and a TTL; add the omitted `hook_ingest` to the socket command list in `docs/design.md`. The per-pane unit test must go through the spool rather than the legacy JSON reader.

TUI changes: none expected; attribution of split-pane sessions becomes correct where it was wrong. List any change in the final summary.

Tests and checks: Spool tests with two panes in one tmux session, ingest followed by cycle, TTL behavior, and the existing hook-sidecar suites.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Two agents in split panes of one tmux session keep separate attribution records
- [ ] #2 The TTL claim in the docs matches the code
- [ ] #3 An ingest never leaves two active hook links for one pane
- [ ] #4 ADR 0028, `docs/operations.md`, and `docs/design.md` match the code
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
