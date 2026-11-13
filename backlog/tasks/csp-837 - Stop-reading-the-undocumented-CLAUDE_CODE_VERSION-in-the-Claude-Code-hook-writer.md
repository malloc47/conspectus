---
id: CSP-837
title: >-
  Stop reading the undocumented CLAUDE_CODE_VERSION in the Claude Code hook
  writer
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 06:05'
updated_date: '2026-11-13 19:10'
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
- [x] #1 The writer either reads a version source Claude Code documents for hooks or stops reading CLAUDE_CODE_VERSION, and the attribution record says nothing it cannot know
- [x] #2 A test covers the chosen behavior
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm no documented version source exists for Claude Code hooks: the hook input's common fields carry none, the environment-variable reference lists no CLAUDE_CODE_VERSION, and the research note found it absent in a 2.1.322 session. The only other carriers (the transcript's lines, `claude --version`, the status line input) are a transcript read, a process start, or not a hook, so none fits `hook write`.
2. Stop reading CLAUDE_CODE_VERSION: the per-harness mapping becomes a pure lookup of the variable each harness's own plugin sets (opencode, Pi), and Claude Code names none, so its attribution record carries no version.
3. Tests: a unit test that Claude Code's record takes no version whatever the environment says, while opencode and Pi still read theirs; an integration test that `hook write claude-code` with CLAUDE_CODE_VERSION set writes a spool record without `harness_version`.
4. Docs: CHANGELOG Fixed entry, the plan README section 8 line.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
No documented version source exists for Claude Code hooks (no hook input field, nothing in the environment-variable reference; the research note found CLAUDE_CODE_VERSION absent in 2.1.322), and the other carriers (transcript lines, `claude --version`) would widen what `hook write` reads or start a process. So the writer stops reading the variable: `harness_version_from` maps only opencode and Pi to their plugin's variable.
Validation: run-some.sh --lib hook_write (13 passed), --test hook_write (25 passed); checks.sh exit 0; run-tests.sh exit 0, 5,629 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`hook write claude-code` no longer reads the undocumented `CLAUDE_CODE_VERSION`. Claude Code documents no version for its hooks (in their input or their environment), so its attribution record now carries no `harness_version` instead of one that was never set; opencode and Pi records keep their plugin's version. The per-harness mapping is a pure `harness_version_from(harness, var)`. Verified by a unit test (Claude Code and Codex take no version whatever the environment holds; opencode and Pi still read theirs) and an integration test (`hook write claude-code` with `CLAUDE_CODE_VERSION=2.1.322` writes a spool record without `harness_version`), checks.sh, and the full suite (5,629 passed). CHANGELOG.md and the plan README's section 8 updated.
<!-- SECTION:FINAL_SUMMARY:END -->
