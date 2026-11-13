---
id: CSP-877
title: 'Match a hook record''s mux by its tmux server, not the session name alone'
status: To Do
assignee: []
created_date: '2026-11-13 20:42'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 954000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The hook sidecar's `find_mux` (`src/discovery/hook_sidecar.rs`) finds a record's mux by `native_id` or session name against the mux node's `native_id`, then by pane pid and cwd. It never looks at the record's tmux socket path. A record from a default tmux server under another `TMUX_TMPDIR` (or another uid's), whose session shares a name with one on the probed default server, attributes its session to the wrong mux; a record from a `-L work` server never matches by name (`work:foo` vs `foo`) and falls through to pid or cwd. The live-state binding now compares the full socket path with the probe's `MuxProbe.socket_path` (ADR 0205's amendment); attribution could use the same rule. Found while doing CSP-853.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A hook record whose socket path names a server other than the mux's probed socket is not attributed to that mux by session name
- [ ] #2 A record from a named (-L) server matches its mux by session name on that server
- [ ] #3 Tests cover a foreign default socket and a -L server record
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
