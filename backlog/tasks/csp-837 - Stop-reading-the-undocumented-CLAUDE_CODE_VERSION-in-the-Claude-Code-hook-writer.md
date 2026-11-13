---
id: CSP-837
title: >-
  Stop reading the undocumented CLAUDE_CODE_VERSION in the Claude Code hook
  writer
status: To Do
assignee: []
created_date: '2026-11-12 06:05'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 922000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook write claude-code` fills the attribution record's `harness_version` from `CLAUDE_CODE_VERSION` (`harness_version_env` in `src/cli/hook_write.rs`). The variable is not in Claude Code's environment-variable reference and was not present in a 2.1.322 session (research note 01, section 0), so the field is in practice always empty while the code and any reader suggest otherwise. Surfaced while building the Claude Code live-state slice; the plan README's section 8 lists it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The writer either reads a version source Claude Code documents for hooks or stops reading CLAUDE_CODE_VERSION, and the attribution record says nothing it cannot know
- [ ] #2 A test covers the chosen behavior
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
