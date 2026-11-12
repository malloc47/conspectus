---
id: CSP-841
title: Count every pane-only agent on its mux
status: To Do
assignee: []
created_date: '2026-11-12 06:57'
labels:
  - live-state
  - ls-bind
milestone: m-26
dependencies:
  - CSP-812
priority: low
ordinal: 926000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`node_live` holds one live binding per node (ADR 0205 decision 2). An entry whose agent has no session node yet binds to the tmux mux it runs in, so when two such agents run in panes of one mux (two fresh Pi or Codex sessions before their first transcript) the mux keeps only the entry the store wrote last, and `live_counts_of_mux` counts one agent instead of two until their sessions appear. Once each agent has a session node, both count. The fix needs a place for several pane-only observations per mux, which is a model change (another side table keyed by mux and pane, or a pane sub-key) and touches the snapshot format, the JSON shape, and the binding in `discovery::live_state`.

Related edge seen while binding: the tmux server of an entry is taken from the last component of its socket path, so a `-S` socket whose file name equals a `-L` server's name, or a default server under another `TMUX_TMPDIR`, would match that server's mux of the same session name.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Two pane-only entries in two panes of one mux both count in live_counts_of_mux
- [ ] #2 A pane-only entry still moves to its session once the session appears
- [ ] #3 Graph JSON and graph.bin carry every pane-only entry, with the format change documented
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
