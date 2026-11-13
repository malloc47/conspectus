---
id: CSP-853
title: Match a pane-only live entry to its tmux server by full socket path
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 14:18'
updated_date: '2026-11-13 20:50'
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
- [x] #1 A pane-only entry whose socket path names a different server than a mux with the same socket name and session name does not bind to that mux
- [x] #2 The default server under another TMUX_TMPDIR or uid does not match the current default server
- [x] #3 Tests cover a `-S` socket named like a `-L` server and a foreign default socket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. tmux session listing gains `#{socket_path}` (before the title, which stays last), so each probe records the full path of the server's socket as the server reports it (`MuxProbe.socket_path`); an older tmux that leaves it blank records none. Snapshot format 16.
2. Live-state binding: a pane-only entry binds to the tmux mux of its session name on the server named by its socket path's last component, as before, and now only when that mux's probe recorded no socket path or recorded exactly the entry's. The pane-title fold uses the same rule to find a pane's hook entry.
3. Tests: a `-S` socket named like a `-L` server, a default socket under another TMUX_TMPDIR or uid, the matching path, the no-path fallback; parser and probe tests for the field.
4. Docs: amendment on ADR 0205 (binding rule 2), ADR index, operations guide, CHANGELOG. File the hook-sidecar attribution analogue (find_mux ignores the socket) as a follow-up.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The path comes from `#{socket_path}` in the existing session listing (no further call), kept on `MuxProbe.socket_path`; checked against a private tmux 3.7c server that it prints the realpath'd socket path tmux also puts in `$TMUX`. A probe without a path keeps the name match. The pane-title fold uses the same server rule. Hook-sidecar attribution has the analogous gap (find_mux ignores the socket); filed as CSP-877. Validation: live_state_tests (default, -L, foreign TMUX_TMPDIR, foreign uid, `-S` named like `-L`, no-path fallback), live_title_tests (a same-named entry on another server is not the pane's hook entry), tmux parser and probe tests. checks.sh green; full suite 5,660 passed, 1 failed (the known "Text file busy" gh spawn flake), which passed alone.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A pane-only live entry now binds only to a mux on the tmux server whose socket it recorded. The session listing asks for `#{socket_path}` (before the title, which stays last), each probe records the path the server reports (`MuxProbe.socket_path`, graph.bin only; snapshot format 16), and binding finds the candidate mux by name as before and then requires an exact path match when the probe has one. The pane-title fold finds a pane's hook entry by the same rule. A tmux that reports no path keeps the old name match. ADR 0205 gains an amendment; operations guide, design, and CHANGELOG updated. The analogous gap in hook-sidecar attribution is filed as CSP-877.

Verified: unit tests for a `-S` socket named like a `-L` server, a default server under another TMUX_TMPDIR and another uid, the matching paths, the no-path fallback, the title fold, and the listing parser/probe; `#{socket_path}` checked on a private tmux 3.7c server. fmt, clippy -D warnings, rustdoc -D warnings, git diff --check pass; full suite 5,660 passed with one known "Text file busy" flake that passed on rerun; tuikit untouched.
<!-- SECTION:FINAL_SUMMARY:END -->
