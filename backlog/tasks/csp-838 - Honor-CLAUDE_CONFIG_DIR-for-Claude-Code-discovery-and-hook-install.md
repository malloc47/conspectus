---
id: CSP-838
title: Honor CLAUDE_CONFIG_DIR for Claude Code discovery and hook install
status: To Do
assignee: []
created_date: '2026-11-12 06:05'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: medium
type: bug
ordinal: 923000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code moves `~/.claude` (settings, projects, transcripts) to `$CLAUDE_CONFIG_DIR` when it is set. Conspectus ignores it: `hook init|status|remove claude-code --scope user` always edit `$HOME/.claude/settings.json`, and discovery looks for transcripts under `~/.claude` unless `CONSPECTUS_CLAUDE_CODE_STATE` is set. An operator who relocates the directory gets hooks installed where Claude Code never reads them and sessions that never appear, with no message saying why. Found by research note 01 (section 4.1) and confirmed while extending the installer to all fourteen live-state events.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With CLAUDE_CONFIG_DIR set, hook init, status, and remove for --scope user use $CLAUDE_CONFIG_DIR/settings.json
- [ ] #2 Discovery finds Claude Code sessions under $CLAUDE_CONFIG_DIR/projects when the variable is set and no Conspectus override is
- [ ] #3 The capability manifest names the new path, and tests run in a scratch home with the variable set
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
