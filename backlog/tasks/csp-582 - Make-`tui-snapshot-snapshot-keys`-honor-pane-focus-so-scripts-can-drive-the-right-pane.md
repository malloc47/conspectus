---
id: CSP-582
title: >-
  Make `tui --snapshot --snapshot-keys` honor pane focus so scripts can drive
  the right pane
status: Done
assignee: []
created_date: '2026-10-02 23:15'
labels:
  - test
milestone: m-11
dependencies: []
ordinal: 280000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Symptom (2026-10-02, while validating `CSP-581`): `--snapshot-keys
  "j<Tab>jjjjj"` was meant to focus the right pane and walk the
  explorer cursor onto a Related row, but every `j` after `<Tab>` still
  moved the left tree selection. Snapshots can only show the right
  pane's default cursor state, so the Preview zone for a Related row
  (evidence list, edge line), Other-zone expansion, drill-down, and
  breadcrumbs can't be checked with the snapshot tool.
- Cause: the interactive loop runs each key through
  `translate(...).and_then(|a| remap_for_focus(a, app.focus()))`
  (`src/tui/runtime.rs`). The snapshot driver's `dispatch_event`
  (`src/tui/snapshot.rs`) returns `translate(event, viewport_height)`
  and skips `remap_for_focus`. `<Tab>` does flip focus, but
  `j`/`k`/`Enter`/`F`/`e`/`g`/`G` are never sent to their `Explorer*`
  messages.
- Scope: apply `remap_for_focus` in the snapshot driver, and route the
  snapshot and interactive paths through one shared helper for key →
  action translation so the two can't drift again. Check whether the
  explorer-only actions that come out of the remap (drill, back, Other
  toggle, full-detail toggle) reach `apply_action`'s supported set and
  aren't skipped as "unsupported action".
- Tests: a snapshot-driver test that runs `<Tab>j…` against a fixture
  and asserts the explorer cursor moved while the left selection
  didn't; one that drills with `<Enter>` and asserts the breadcrumb.
  Update the CLAUDE.md snapshot guidance with a right-pane example
  once it works.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-10-02): `runtime::action_for_event` is now the one
key → action path (overlay routing, `translate`, `remap_for_focus`),
used by both the interactive loop and the snapshot driver. That also
retired the driver's own overlay list, which had drifted: it missed
the worktree menu, new-mux form, mux menu, and mux launch form, and
checked overlays in a different order. Right-pane `Enter` runs the
same branch as live (`runtime::explorer_enter`) with the OSC 52
clipboard write skipped so it can't leak into the frame; drill-down,
`Backspace`, the Other toggle, and full detail were already plain
`Msg`s. Driver tests cover `<Tab>j…` moving only the explorer cursor
and `<Enter>` / `<Backspace>` drilling and popping a hop; both fail
without the remap. AGENTS.md, README, and `docs/dev-scenarios.md`
gained right-pane examples.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-008`
