---
id: CSP-836
title: Bound the daemon socket calls the TUI and CLI make
status: To Do
assignee: []
created_date: '2026-11-12 04:32'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
ordinal: 921000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook write`'s `hook_ingest` call now times out after 2 s on each read and write, but `status`, `refresh`, and `snapshot` still wait indefinitely on the daemon socket. A daemon that accepts a connection and never answers (stuck on its writer lock, or wedged) hangs `conspectus status`, a CLI command that asks the daemon for its snapshot, and the TUI's snapshot fetch. A `refresh` can legitimately take as long as a full rebuild, so one bound does not fit every command; each needs a bound that fits it, or a way for the caller to give up and fall back to local discovery.

Context: `call_socket` in `src/server/mod.rs` already takes a per-call timeout; `call_command` passes none. Surfaced while making `hook write` silent and bounded.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A daemon that accepts and never answers no longer hangs the snapshot fetch of the TUI or a one-shot command; each falls back as it does when no daemon runs
- [ ] #2 `conspectus status` and `refresh` either finish or report a timeout within a documented bound
- [ ] #3 Tests use a listener that never replies
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
