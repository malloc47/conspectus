---
id: CSP-219
title: Audit harness control planes for non-mutating current-session queries
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 256000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: determine whether any supported harness exposes a
  documented side-channel that can ask an already-running
  interactive process for its current session id without entering
  text into the conversation or mutating JSONL/session logs. Audit
  Codex `app-server` / `app-server proxy` / `--remote`, opencode
  `serve` / `attach` / `acp`, Claude Code remote-control /
  background-agent surfaces, and aider if a relevant server mode
  exists. For each harness, record: launch mode required, discovery
  path for the socket/URL/token, query shape, mutation guarantees,
  authentication boundaries, and fallback behavior when the
  control plane is absent.
- Tests: none for the audit itself. If a supported control plane
  survives, create follow-up fixture or fake-server tests before
  implementing the adapter.
- Manual checks: launch each harness in the required server/control
  mode and prove the query does not append user, assistant, or
  system records to the session transcript.
- Related: `CSP-227` captures a live Claude Code case where
  command-line `--resume` evidence became stale after an in-process
  session switch; the audit should explicitly look for a safer
  current-session source for that scenario.
- Blockers: none.
- **audit slice landed**: local Codex CLI exposes experimental
  app-server thread APIs, but no local hook surface in `--help`;
  keep `CSP-220` gated. OpenCode exposes HTTP server, ACP,
  and plugin surfaces; keep `CSP-221` gated. Claude Code's
  strongest non-mutating path is hooks, tracked under
  `CSP-223` / `CSP-226`.
- **closed 2026-05-31**: audit work is the scope; the recorded
  findings have routed each harness to its chosen non-mutating
  path (Codex → ADR 0048 log linker; opencode → plugin sidecar
  via `CSP-229`; Claude Code → hook sidecar via
  `CSP-226`). `CSP-220` and `CSP-221` are
  closed as won't-do; see their entries for rationale.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-005`
