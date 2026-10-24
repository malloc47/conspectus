---
id: CSP-704
title: Say when the daemon snapshot is unreadable instead of silently rebuilding
status: To Do
assignee: []
created_date: '2026-10-24 04:10'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
type: bug
ordinal: 780000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a client gets snapshot bytes from `conspectus serve` that it can't decode (most often a format version mismatch after the daemon is upgraded and an older TUI or CLI is still running), `try_daemon_snapshot` in `src/tui/runtime.rs` and its twin in `src/cli/mod.rs` return `None`. The client then quietly rebuilds the whole graph itself, exactly as if no daemon were running. Nothing tells the operator why, and the TUI also writes its own `graph.bin` in the old format over the daemon's.

Seen on 2026-10-23: after a daemon restart onto snapshot format 9, a TUI still running an Oct 20 binary (format 7) took 124 s to first paint on the operator's data (114 sessions, 13 muxes) and about 30 s per refresh after that. Every mux the operator detached from sat frozen behind the hand-off spinner until that slow refresh landed, and it looked like the TUI was broken. The fix was to restart the TUI, but nothing pointed there. The same thing will happen to today's binary the next time `FORMAT_VERSION` changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a running daemon serves a snapshot the client cannot decode, the TUI shows a visible warning that names the mismatch (for example the daemon is newer and the TUI needs a restart), not only a silent fallback
- [ ] #2 The one-shot CLI prints one stderr warning in the same case and still produces its output
- [ ] #3 A client that cannot decode the daemon snapshot does not overwrite `graph.bin` with its own format
- [ ] #4 An unavailable daemon (no socket, first cycle not done) keeps today's quiet fallback; only an undecodable snapshot warns
- [ ] #5 Tests cover a version-mismatched snapshot for both the TUI and CLI paths
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
