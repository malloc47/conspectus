---
id: CSP-042
title: Verify the Phase 3 end state
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-040
  - CSP-041
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: run the full Phase 3 automated and manual check set and record any
  follow-up tasks instead of expanding Phase 3 scope.
- Tests: `just check`.
- Manual checks: run the tmux smoke commands from the Phase 3 plan and run
  against real local harness state if available.
- Blockers: `CSP-040`, `CSP-041`.
- Follow-up: real codex/claude/opencode state did not populate
  `agent_session.cwd`, so `cross_link::infer` never matched the smoke
  session against the live harness data. The Phase 3 adapters parse the
  synthetic fixture shapes; aligning them with the actual production
  JSONL/info.json layouts (and propagating cwd plus activity epochs)
  belongs in a Phase 4-or-later task rather than expanding Phase 3.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`nix develop --command just check` passed with 124 tests. The
tmux smoke test (`tmux new-session -d -s conspectus-smoke -c "$PWD"` +
`cargo run -- graph --format json`) emitted one repo/checkout/branch,
three mux sessions (including the smoke session at the conspectus repo
cwd) and 16 agent sessions from the real `~/.codex`, `~/.claude`, and
`~/.local/share/opencode` state. Discovery remained read-only.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-011`
