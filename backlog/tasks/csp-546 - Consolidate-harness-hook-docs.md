---
id: CSP-546
title: Consolidate harness-hook docs
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 617000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `docs/operations.md` documents only the Claude Code hook;
  `conspectus hook init` also supports codex; the opencode plugin
  (`plugins/opencode-hook`, ADR 0049) and the older
  `scripts/conspectus-claude-hook-sidecar.py` need one "Harness hooks"
  section, or the script retires in favor of `conspectus hook write`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-015`
