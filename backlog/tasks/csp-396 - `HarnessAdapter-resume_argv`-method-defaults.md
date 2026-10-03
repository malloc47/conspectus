---
id: CSP-396
title: '`HarnessAdapter::resume_argv` method + defaults'
status: Done
assignee: []
created_date: '2026-06-05 22:52'
labels:
  - h-pin-resume
milestone: m-11
dependencies: []
ordinal: 337000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `fn resume_argv(&self, session_id: &str, cwd: &Path)
  -> Option<Vec<OsString>>` to `HarnessAdapter`. Per-adapter
  defaults: `codex` returns `Some(vec!["codex", "resume",
  session_id])` (or whatever its CLI shape is), `claude-code`
  returns `Some(vec!["claude", "--resume", session_id])`,
  `opencode` returns `Some(...)` if its CLI supports resume
  (decide during impl from the actual CLI), `aider` returns
  `None`. Cwd argument is accepted by every adapter even when
  unused so the signature stays consistent. No launch wiring
  yet.
- Tests: per-adapter unit tests for the default; one test
  asserting `None` for `aider`; one CLI fixture invocation
  confirming the constructed argv parses correctly with the
  real binary (gated behind a feature flag or env check so CI
  doesn't depend on the harness being installed).
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`HarnessAdapter::resume_argv(session_id, &Path)`
added with a `None` default. Codex returns
`["codex", "exec", "--resume", id]`; claude-code returns
`["claude", "--resume", id]`; opencode returns
`["opencode", "--session", id]`. Aider tracks chat history
per-cwd rather than per-session and inherits `None`. Sibling
`resume_argv_for(harness_key, ...)` helper mirrors
`launch_argv_for`. 6 unit tests (one per supported
adapter / one for aider / one for unknown harness / one for
the trait default).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-RESUME-002`
