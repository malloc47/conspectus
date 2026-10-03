---
id: CSP-330
title: Surface recall's harness coverage gap (or broaden it)
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-transcript
  - wont-do
milestone: m-11
dependencies: []
ordinal: 205000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Won't fix on the conspectus side** as of ADR 0052 — the native viewer (H-VIEWER-NATIVE-*) reads opencode directly via SQLite (ADR 0013), so the gap is closed by replacing the backend rather than patching recall. Operators who still want recall as the opencode viewer maintain the patches themselves.

- Scope: on at least one observed machine, recall hard-codes
  `~/.local/share/opencode/storage/session` but the real
  OpenCode storage on the same machine lives at
  `~/.local/share/opencode/storage/session_diff/` (and the
  SQLite-of-record is `~/.local/share/opencode/opencode.db`).
  The result: `recall list --source opencode` returns empty
  even though conspectus discovers many opencode sessions
  locally, and `recall --session <opencode-id>` exits 1 with
  "Session not found". CSP-216's stderr-hold makes
  the failure visible, but the underlying gap stays.
- Resolution options (pick one in the story):
  (a) **Document** and leave: opencode-via-recall is an
      unsupported combination on machines whose opencode store
      lives outside recall's hardcoded path. The TUI status
      bar already explains why on failure.
  (b) **Extend the recall patch** to add a search-roots flag
      (`--scan-root <path>` repeatable) and pass conspectus's
      resolved `state_scope` per-harness.
  (c) **Upstream PR** to point recall's opencode parser at the
      real layout (session_diff/ + opencode.db), then drop the
      local-only workaround.
- Tests: viewer-resolver tests already cover the disabled
  paths; this story would add an integration probe that runs
  `recall list --source <harness>` against the actual machine
  layout when one is available, so coverage gaps regress
  visibly.
- Blockers: none. Independent of `CSP-332`'s config
  override (which would also let users sidestep the gap by
  swapping recall for an opencode-native viewer per-harness).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-014`
