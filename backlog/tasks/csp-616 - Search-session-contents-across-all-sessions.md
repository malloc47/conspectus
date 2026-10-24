---
id: CSP-616
title: Search session contents across all sessions
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-24 04:47'
labels:
  - h-search
milestone: m-11
dependencies: []
priority: medium
type: feature
ordinal: 665000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The `/` overlay finds sessions and muxes by their metadata (alias, title, cwd, last-message preview), but there is no way to find a session by what was said in it. The operator wants to search the contents of every session ("which session did I discuss X in?"). Conspectus already parses Claude Code, Codex, and opencode transcripts for the native viewer (`src/viewer/parser/`, reached through `HarnessAdapter::transcript_source` and `transcript_parser`), so the reading side exists.

Operator decisions (2026-10-16):
- The MVP scans transcripts on demand, with no index. A persistent full-text index is wanted eventually and is filed as follow-up subtasks, together with the ADR 0086 amendment it needs (ADR 0086 forbids caching transcript content beyond the current session).
- v1 ships three surfaces: a contents scope in the `/` overlay, find and jump-to-match in the transcript viewer, and a `conspectus search` CLI.
- Results can be grouped per session (the default) or per message, and the operator can switch between them.
- Searched content is configurable: user and assistant messages by default, with tool calls and output, and thinking, as opt-ins.

Measured 2026-10-16 on the operator's machine: about 320 MB of transcripts (26 Claude Code files, median 1.7 MB and largest 31 MB; 51 Codex rollouts; one opencode SQLite DB). A raw byte scan takes about 50 ms and a single-threaded JSON parse of every record about 0.9 s, so an on-demand scan is fast enough at this size.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An operator can find every session that mentions a phrase from the TUI and from the CLI
- [ ] #2 Opening a result lands on the matching turn in the transcript viewer
- [ ] #3 Grouping (per session or per message) and searched content kinds are configurable, defaulting to per session and messages only
- [ ] #4 No transcript content is persisted or sent through the daemon (ADR 0086)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
New operator requirement (2026-10-23): search must work both as a dedicated search view and as a filter on the session list. Added CSP-616.10 (content filter) and CSP-616.11 (Search view). CSP-703 is a prerequisite of CSP-616.05. CSP-616.07 to .09 (persistent index) stay deferred behind the latency/corpus thresholds, except .07 which records the decision.
<!-- SECTION:NOTES:END -->
