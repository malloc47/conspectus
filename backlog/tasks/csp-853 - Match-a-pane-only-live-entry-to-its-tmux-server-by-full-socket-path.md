---
id: CSP-853
title: Match a pane-only live entry to its tmux server by full socket path
status: To Do
assignee: []
created_date: '2026-11-12 14:18'
labels:
  - live-state
  - ls-bind
milestone: m-26
dependencies: []
priority: low
ordinal: 938000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`discovery::live_state` binds an entry that no session matches to the tmux mux it names, taking the server from the last component of the entry's socket path (the name `tmux -L` gives it, `default` for the default server) and matching it against the mux node's socket name. Two different servers can therefore look alike: a `tmux -S /some/dir/work` socket whose file name equals a `-L work` server's name, or a default server under another `TMUX_TMPDIR` (or another uid's `/tmp/tmux-<uid>/default`), would bind a pane-only agent to the mux of the same session name on the wrong server and count it there. Seen while doing CSP-841. Mux nodes record only the socket name today, so the fix likely needs the mux's full socket path in the graph; CSP-386 (absolute `-S` socket paths) is the related identity work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane-only entry whose socket path names a different server than a mux with the same socket name and session name does not bind to that mux
- [ ] #2 The default server under another TMUX_TMPDIR or uid does not match the current default server
- [ ] #3 Tests cover a `-S` socket named like a `-L` server and a foreign default socket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
