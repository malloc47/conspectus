---
id: CSP-840
title: Parse only the whitelisted fields of a hook payload in hook write
status: To Do
assignee: []
created_date: '2026-11-12 06:06'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 925000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook write` parses the whole stdin payload into a `serde_json::Value` before any adapter sees it, and every `HarnessAdapter` hook method takes that `Value`. The Claude Code mapping then reads only a few named fields, but the full parse has already allocated every string, including a `PostToolUse` `tool_response` that can run to megabytes (a large file read, a long command output), on every tool call. Research note 01 (sections 0 and 4.2) asks for a typed whitelist parse that skips the rest; the adapter side is done, the writer side is not.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hook write never builds a full Value of a payload; unknown fields are skipped during parsing
- [ ] #2 Every harness adapter keeps its current mapping, attribution, and privacy tests green
- [ ] #3 A test shows a payload with a multi-megabyte tool_response costs about what a small one does
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
