---
id: CSP-859
title: Update the snapshot key examples that walk onto a Related row
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 00:24'
updated_date: '2026-11-14 02:45'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
ordinal: 944000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The project instructions (CLAUDE.md and AGENTS.md), README.md, and docs/dev-scenarios.md give `--snapshot-pane right --snapshot-keys 'j<Tab>jjjjj'` as the way to walk the right pane's cursor past the Node fields onto a Related row. Since ADR 0213 (CSP-829) a session or mux with live state lists up to five more fields (`state`, `since`, `source`, `evidence`, `subagents`, or `agents` and `pane_agents`), so on such a node the example stops on a field instead of a Related row. The showcase screenshot recipe and test were updated; the prose examples were not, and CLAUDE.md is the operator's file to edit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every documented example that claims to reach a Related row does so on a session with live state, or says how many fields it walks past (README.md, docs/dev-scenarios.md, AGENTS.md)
- [ ] #2 CLAUDE.md is changed only by the operator or with the operator's explicit approval
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. The explorer has no next-section key (EXPLORER_KEYS: move, page, first/last, open, fold, inspect, show all, back), so a walk must count fields. Pin the examples to the showcase fixture, where the first session (showcase-deck-launcher, live state) lists nine Node fields (id, harness, title, cwd, status, then state, since, source, evidence) and `j<Tab>jjjjjjjjj` reaches its first Related row.
2. README.md and docs/dev-scenarios.md: add `--snapshot-fixture tests/fixtures/showcase.json`, walk nine fields, say how many fields it walks past and that another node needs another count.
3. AGENTS.md (CLAUDE.md): write the same change as /tmp/csp-fu/agents-md-859.patch for the operator; do not commit it.
4. Verify each documented command by running it against the showcase in a scratch environment; git diff --check, checks.sh, just-check.sh.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
README.md and docs/dev-scenarios.md updated (commit 3210d423): the examples run on the showcase fixture and walk `j<Tab>jjjjjjjjj` past the first session's nine Node fields (id, harness, title, cwd, status, then state, since, source, evidence) onto `associated with`; dev-scenarios adds `<Enter>`, which drills into the workspace. Both say how many fields they pass and that another node lists a different number. The explorer has no next-section key (EXPLORER_KEYS: move, page, first/last, open, fold, inspect, show all, back), so no count-free walk exists.
Verified by running both commands against the showcase in a scratch environment (fixture only, and with fixture root and pinned clock) at 80, 150, and 200 columns. checks.sh clean; just check passed (5706 passed).
WAITING: the AGENTS.md (CLAUDE.md) line waits on the patch /tmp/csp-fu/agents-md-859.patch (same walk, same wording; `git apply --check` passes), left for the operator to apply or approve. Criterion 1 stays unchecked until it lands; criterion 2 holds on this branch (AGENTS.md untouched) and is checked when the patch lands with the operator's approval.
<!-- SECTION:NOTES:END -->
