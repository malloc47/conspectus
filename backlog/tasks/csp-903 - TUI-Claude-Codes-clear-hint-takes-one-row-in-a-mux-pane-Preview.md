---
id: CSP-903
title: 'TUI: Claude Code''s /clear hint takes one row in a mux pane Preview'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-18 13:12'
updated_date: '2026-11-18 13:44'
labels:
  - tui
dependencies:
  - CSP-902
ordinal: 979000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code right-aligns a hint on its status line, `new task? /clear to save 494.6k tokens`, behind a run of spaces as wide as most of the pane. In a narrower Preview the indent alone leaves no room, so smart wrap (whose hanging indent gives up past half the width) shows a row of blanks and then the text on a second row. Found 2026-11-18 while verifying CSP-900. The operator wants it right-aligned within the Preview on one row, and as a configured rule rather than code (CSP-902).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under the built-in settings, Claude Code's `/clear` hint shows on one row of the mux Preview, flush with its right edge, its text whole
- [x] #2 The rule is a built-in `[[tui.preview.rules]]` entry scoped to Claude Code panes, which config can override or turn off by name
- [x] #3 Other panes, and Claude Code lines the rule does not match, lay out as before
- [x] #4 A test covers the hint at a Preview narrower than its indent, and a snapshot of a real capture shows it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. The fix is the built-in rule `claude-code-clear-hint` in `src/tui/preview_defaults.toml`: scoped to `claude-code`, matching the hint behind its padding, fit `right-align` (CSP-902 supplies the mechanism).
2. Tests: the rule's pattern matches the padded hint and not the same words as content; a Codex pane leaves the hint to smart wrap; the rule scoped to the pane's harness right-aligns it on one row.
3. Snapshot a real Claude Code capture through the snapshot harness.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The built-in rule `claude-code-clear-hint` (scoped to `claude-code`, `match = '^\s+new task\? /clear to save \S+ tokens\s*$'`, `fit = "right-align"`) puts Claude Code's `/clear` hint on one row flush with the Preview's right edge, where smart wrap gave a row of blanks and then the text. Config can replace or turn it off by name. Verified by the rule's pattern test (the padded hint matches; the same words as content do not), a Preview test showing a Codex pane leaves the hint to smart wrap and the rule scoped to the pane right-aligns it at 60 columns, and a snapshot of a live Claude Code capture showing the hint on one row at the right edge.
<!-- SECTION:FINAL_SUMMARY:END -->
