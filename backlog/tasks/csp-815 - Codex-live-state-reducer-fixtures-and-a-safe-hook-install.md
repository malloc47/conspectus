---
id: CSP-815
title: 'Codex live state: reducer, fixtures, and a safe hook install'
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:27'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-810
  - CSP-811
documentation:
  - docs/plans/live-state/research/02-codex.md
  - >-
    docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: medium
type: enhancement
ordinal: 901000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Codex now has a full hook system whose entries sit beside the operator's own, but the installed Conspectus entry likely does nothing on current Codex until the operator trusts it in `/hooks`, and the installer replaces an operator's `[[hooks.SessionStart]]` tables and sets no timeout. State needs the installer fixed and the reducer written.

Scope: Fix `hook init codex`: preserve the operator's array-of-tables and inline forms, set a timeout on every entry, install the Tier A set of ADR 0206 as separate matcher groups, make status and remove idempotent and exact, and print the `/hooks` review instruction. Never write a trust value. Implement the Codex `LiveEvent` mapping and reducer (`02-codex.md` section 6) including the subagent child set and `permission_mode` plan handling, fixtures from the documented shapes with sentinel text, and the `unknown event is ignored` behavior.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Install tests over: operator tables, inline hooks, a `hooks.json` beside `config.toml`, a present `hooks.state`, remove with Conspectus's group between operator groups; reducer sequence tests; privacy sentinels.

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `hook init codex` preserves operator hooks in every supported form and sets timeouts
- [ ] #2 The Tier A events map to the states in the research table, with a sequence test per scenario
- [ ] #3 `hook init codex` tells the operator to review the hook in `/hooks`, and nothing writes a trust value
- [ ] #4 `hook remove codex` removes only Conspectus's groups
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
