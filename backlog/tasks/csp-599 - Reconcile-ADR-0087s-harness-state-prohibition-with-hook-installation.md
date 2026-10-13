---
id: CSP-599
title: Reconcile ADR 0087's harness-state prohibition with hook installation
status: To Do
assignee: []
created_date: '2026-10-13 03:24'
labels:
  - h-caps
milestone: m-11
dependencies: []
priority: medium
ordinal: 648000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Writing the capability manifest (ADR 0117) surfaced a conflict. ADR 0087's first prohibition lists `~/.claude/` as harness-owned state Conspectus never writes, yet ADR 0028's `conspectus hook init|remove` edits `~/.claude/settings.json` and `~/.codex/config.toml`, with `--scope project` variants.

The manifest records this as its own `hook` context so the tests pass. Decide one of:
- amend ADR 0087 to sanction operator-initiated hook installation as a category;
- narrow hook install, for example to print-only.

Update the manifest and docs to match.

Blockers: an operator decision.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
