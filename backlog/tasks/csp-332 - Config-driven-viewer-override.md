---
id: CSP-332
title: Config-driven viewer override
status: To Do
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 207000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the hard-coded `ClaudeHistoryViewer` /
  `RecallViewer` resolver with a `[viewers.<harness>]` (and
  `[viewers.default]`) config section so users can bring their
  own opinions without touching Conspectus internals. The
  `recall --session` patching in `pkgs/recall/` was the
  immediate driver — a future user shouldn't have to fork
  Conspectus to swap in `cass view`, `claude-code-log`, a
  homegrown wrapper, etc.
- Config shape: each entry is `command = ["prog", "arg", ...]`
  with `{placeholder}` interpolation. Conspectus expands at
  launch time. Precedence: project config > user config >
  built-in resolver. Per-harness entry wins over
  `[viewers.default]`. User-configured entries **bypass
  `required_flags` probing** — the operator has opted in and
  accepts whatever the child process does.
- Placeholder set (frozen by the ADR below):
  `{session-id}` (raw `AgentSessionId.session_key`),
  `{session-file}` (on-disk transcript path; empty / launch
  refused for harnesses that have no single file — opencode
  SQLite, codex split state/log per ADR 0048),
  `{cwd}` (session cwd or empty),
  `{harness}` (`AgentSessionId.harness_key`),
  `{state-scope}` (`AgentSessionId.state_scope`).
- ADR: small ADR freezes the placeholder names, the precedence
  rules, the unknown-placeholder behavior (diagnostic +
  leave-as-text vs refuse-to-launch), and the
  `{session-file}` semantics for SQLite-backed harnesses.
  Variable names become a public contract once shipped.
- Implementation outline: new `ViewersConfig` in `src/config.rs`
  mirroring the `TuiViewsConfig` parse/merge shape (~120 LOC),
  a `ConfiguredViewer` backend in `src/tui/viewer.rs` with a
  template expander (~80 LOC), resolver order change (~20 LOC),
  a per-harness `state_file_for(session_id)` mapper (deciding
  Option A: derive on demand vs. Option B: add an optional
  `state_file` field to `AgentSessionNode`; deferred to the
  ADR). Tests cover precedence, placeholder expansion,
  `{session-file}` for the SQLite-backed harnesses, and the
  interaction with the existing `required_flags` gating
  (configured viewer bypasses it).
- Tests: config parse/merge for the new sections; template
  expander with known/unknown placeholders; resolver tests
  that a configured entry wins over the built-in backends; a
  refused-launch test for `{session-file}` against opencode.
- Blockers: none (independent of the inline-widget track).
  Should land before `CSP-216` ships outside the
  author's machines so the patched-recall workaround is
  optional rather than the only path.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-013`
