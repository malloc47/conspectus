---
id: CSP-478
title: Provide transcript locator and parser via the adapter
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-474
ordinal: 160000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. `SessionLocator` flattened from a
  closed `enum { ClaudeCode { .. }, Codex { .. }, OpenCode { .. } }`
  to an open `struct { harness_key, session_key, state_root }`.
  Each parser interprets `state_root` per its own convention
  (claude-code + codex: harness state root; opencode: SQLite
  database path). `HarnessParser` trait sheds its per-parser
  `supports(&locator) -> bool` filter — the adapter registry
  now dispatches by harness key so per-parser filtering is
  dead weight.
  `HarnessAdapter` gains two trait methods:
  `transcript_source(&AgentSessionId) -> Option<SessionLocator>`
  and `transcript_parser() -> Option<&'static dyn HarnessParser>`.
  Both default to `None`; adapters that ship a native viewer
  surface override. claude-code, codex, opencode wire their
  parsers and build their locators (opencode's
  `state_scope` → `opencode.db` resolution moves from
  `viewer_bridge` onto the adapter). Aider inherits the
  default `None` — explicit "no transcript source" answer
  that flows through the bridge and drops into the
  escape-hatch external viewer.
  `viewer_bridge::locator_for_session` collapses to a two-line
  registry lookup. `build_viewer_state` grabs the same
  adapter's parser and delegates the read. The pre-H-EXT-006
  fixed `[&ClaudeCodeParser, &CodexParser, &OpenCodeParser]`
  dispatch array is gone.
  Serde shape changed from tagged-enum
  (`{"harness": "claude-code", "state_root": ..., "session_key": ...}`)
  to flat struct
  (`{"harness_key": "claude-code", "session_key": ..., "state_root": ...}`).
  No persisted SessionLocator anywhere in the codebase, so
  this is a pure test-shape change.
  Existing parser + bridge tests migrated to the flat shape;
  three `supports_only_*` tests deleted as redundant. All
  25 suites (1516 lib tests) pass; fmt / clippy clean.
- Blockers: `CSP-474` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-006`
