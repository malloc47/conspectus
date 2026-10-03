---
id: CSP-220
title: >-
  Add Codex app-server attribution adapter if the audit proves a stable
  non-mutating query
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-219
ordinal: 257000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Closed as won't-do 2026-05-31.**

- Scope: if `CSP-219` confirms Codex's app-server or control
  socket can report the active session/rollout for an interactive
  TUI, implement an optional adapter that discovers the control
  endpoint, authenticates using the documented local mechanism, and
  emits high-confidence `LinkedToMux` evidence for the active
  session. This should be a launch-mode enhancement only: existing
  plain TUI sessions must continue to rely on fd / command / cwd
  evidence.
- Tests: fake app-server protocol tests for current-session,
  missing-session, auth failure, server unavailable, and stale
  socket cases.
- Manual checks: launch Codex with the required app-server mode;
  confirm Conspectus links the live rollout without relying on
  command-line resume args or open JSONL fd paths.
- Blockers: `CSP-219`.
- **closure rationale**: `CSP-219` audit found Codex's
  app-server surface is gated behind experimental flags with no
  stable contract and no local hook surface (`codex --help`). The
  Codex drift class that motivated this work is already covered by
  the log linker landed under `CSP-218` / ADR 0048, which
  derives current session attribution from the on-disk rollout log
  without depending on the experimental control socket. Reopen
  only if Codex ships a stable, documented current-session query
  that the log linker cannot match (e.g. cross-pid session
  handoff without log rotation).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-006`
