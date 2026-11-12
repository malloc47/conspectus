---
id: CSP-819
title: Keep the operator's key order when hook install edits settings.json
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies:
  - CSP-811
documentation:
  - docs/plans/live-state/research/01-claude-code.md
priority: low
type: enhancement
ordinal: 905000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook init` and `hook remove` rewrite Claude Code's `settings.json` through a JSON builder that sorts object keys, so the operator's file is reordered on the first install. Values survive, but a reordered settings file produces a noisy diff in a dotfiles repository.

Scope: Enable order preservation for the settings edit. `serde_json`'s `preserve_order` feature adds `indexmap` as a dependency, so this needs an ADR first (need, alternatives such as editing the text in place, the dependency's cost), then the change, with the existing install, upgrade, and remove tests asserting byte-stable output for untouched keys.

TUI changes: none.

Tests and checks: Install, upgrade, and remove on a scratch settings file with a deliberately unsorted key order and formatting, comparing untouched sections byte for byte.

Blockers: The Claude Code install.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR records the need and the alternatives
- [ ] #2 `hook init` and `hook remove` leave untouched keys in their original order
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
