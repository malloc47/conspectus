---
id: CSP-877
title: 'Match a hook record''s mux by its tmux server, not the session name alone'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 20:42'
updated_date: '2026-11-14 01:32'
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
- [x] #1 A hook record whose socket path names a server other than the mux's probed socket is not attributed to that mux by session name
- [x] #2 A record from a named (-L) server matches its mux by session name on that server
- [x] #3 Tests cover a foreign default socket and a -L server record
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Share the binding's server rule (server name from the socket path's last component, then the probe's full socket path where it read one; live_state.rs) as one helper, `on_server(snapshot, mux, socket)`.
2. find_mux: when the record names a socket path, its session-name match and its cwd fallback consider only muxes of that server; with no socket path the old name match (native id / session name against the mux's native id) and cwd fallback stay. The pid match is unchanged.
3. Tests in hook_sidecar_tests.rs: a record from a foreign default socket (same session name, same cwd) is not attributed; the probed socket and a probe without a path still match; a -L server record matches its mux by session name, beside a default-server mux of the same name.
4. Amend ADR 0028 (matching order) and the sentence in ADR 0205's amendment that says attribution ignores the socket; CHANGELOG, operations if it states the rule.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The record's server also bounds the directory fallback: a record from a server Conspectus does not list (or a foreign default socket) would otherwise get the same strong hook link on any of your muxes in its directory, which is the bug in another form. The pid match is left alone: a mux whose active pane runs the record's process is that process's. Verified: the three new wrong-server tests fail on the old find_mux and pass now; `just check` 5698 passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`find_mux` (src/discovery/hook_sidecar.rs) now matches a record that names its tmux socket path only to a mux of that server, by the binding's rule shared as `live_state::on_server`: the path's last component names the server, and the probe's `#{socket_path}` must equal the record's where the probe read one. The session name is compared with the mux's bare session name, so a `-L work` record matches `work:<name>`; the directory fallback stays on the record's server. A record with no socket path keeps the old name match, and a probe with no path leaves the name to decide; the pid match is unchanged.

Tests (hook_sidecar_tests.rs): a foreign default socket with the same session name and directory gets no link while the probed socket's record links; a probe without a path still matches by name; a `-L` record and a default record, both naming session `editor`, link to their own servers' muxes; a `-L` record whose session is gone falls back to a mux in its directory on its server, not the default server's. Three of them fail against the old code. ADR 0028 amended (and the sentence in ADR 0205's amendment), design.md, operations.md, CHANGELOG. `checks.sh` and `just check` (5698 passed) green.
<!-- SECTION:FINAL_SUMMARY:END -->
