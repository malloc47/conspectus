---
id: CSP-206
title: Resolve ADR 0019 with the May 2026 survey
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 194000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0019 amended in place to Accepted. Survey
refreshed: `ccview` is alive (v1.0.1, April 2026) — the
earlier "disappeared" note was incorrect; `claude-history`
(v0.1.64, May 2026) gained a structured agent protocol and
is the preferred Claude Code launch backend; `recall`
(v0.5.0, Jan 2026) is the preferred multi-harness backend
for Claude/Codex/OpenCode/Factory; `lazyagent` v0.12.0 has
an HTTP API (deferred — long-running service, not a
one-shot viewer); `ccboard-core` is published on crates.io
at 0.22.0; `cass`
(`coding_agent_session_search`) covers 20+ providers with
`--json`/`--robot` but is deferred pending license-rider
review. The trait was narrowed from
`SessionViewer::{view, export_text}` to a
`SessionViewerAction::plan -> LaunchPlan` action-resolver
seam mirroring `CSP-169`'s `tmux attach` hand-off. The ADR
explicitly calls out that the inline preview is handled by
ADR 0051 and the rest of the `H-TRANSCRIPT-*` workstream.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TRANSCRIPT-002`
