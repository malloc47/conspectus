---
id: CSP-911
title: 'TUI: say why the mux this TUI runs in has no preview'
status: To Do
assignee: []
created_date: '2026-11-25 20:04'
labels:
  - tui
dependencies: []
references:
  - src/tui/ui/preview_pane.rs
  - docs/adr/0258-tui-the-mux-this-tui-runs-in-previews-its-agents-transcript.md
priority: low
type: enhancement
ordinal: 986000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The tmux session Conspectus runs in never previews its pane, because the pane would be the TUI drawn inside itself. When it has nothing else to show (the default `pane` choice, or `mux_preview = "transcript"` with no agent the viewer can read), its row says `no mux target for this row`. That wording describes it loosely: the row is a mux and it has a target in tmux's sense, it just cannot be previewed or attached to from here. An operator looking at it cannot tell it apart from a mux that failed to resolve.

The string is shared with other rows that have no mux to capture (a stopped mux with no session to show, a pin whose mux is gone), which is why ADR 0258 left it alone. A clearer message for this one case means telling that row apart from those.

The user-facing text rules apply: nothing the operator reads names an ADR, a backlog ID, or a design document.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The mux that hosts this TUI, when it has no preview, says that it is the tmux session Conspectus is running in, in the Mux view, on a pin's row in the Pinned view, and on a drilled-in mux
- [ ] #2 Every other row that says `no mux target for this row` today still says it, and a hosting mux that has a transcript to show is unchanged
- [ ] #3 The text follows the rules on user-facing text and passes the text consistency tests
- [ ] #4 Tests cover each place above and a rendered frame; `just check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
