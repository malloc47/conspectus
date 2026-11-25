---
id: CSP-912
title: 'TUI: a session row in the TUI''s own mux previews nothing under the pane choice'
status: To Do
assignee: []
created_date: '2026-11-25 20:04'
labels:
  - tui
dependencies: []
references:
  - src/tui/preview/mod.rs
priority: low
type: bug
ordinal: 987000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
With `[tui] session_preview = "pane"`, a session in tmux previews its pane. If that session's mux is the tmux session Conspectus runs in, the pane would be the TUI itself, so the Preview falls back to the session's transcript. The two places that resolve a session's Preview disagree about it.

`node_source` (`src/tui/preview/mod.rs`) drops the pane when the mux hosts this TUI and returns the transcript, so the same session reached as a drilled-in node or a Related neighbor previews its transcript. `selection_source` resolves a session row in tmux under the pane choice through `resolve_attach_target`, which refuses the TUI's own session, so the row previews nothing.

Reproduced on 2026-11-25 with a throwaway test, in a world with one codex session in the tmux session `editor`, `session_preview = Pane`, and `current_tmux_session = "editor"`: `preview_source` for the session's row returned `None`, and `node_source` for the same session returned `Transcript(codex:/state:abc)`. The convention that the same entity previews alike in every view says the row should show the transcript.

Found while building CSP-910, which makes the mux itself preview its agent's transcript and left this older inconsistency alone.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With `session_preview = "pane"`, a session row whose mux is the tmux session Conspectus runs in previews the session's transcript, the same source `node_source` gives that session
- [ ] #2 A session row in any other mux, and one outside tmux, resolves as before under both choices
- [ ] #3 The pane of the TUI's own mux is still never previewed or captured, and attaching to it is still refused
- [ ] #4 Tests cover the row and the node resolving alike, the unchanged rows, and a rendered frame; `just check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
