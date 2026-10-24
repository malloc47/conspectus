---
id: CSP-701
title: 'Remove aider from operator docs, README, and changelog'
status: To Do
assignee: []
created_date: '2026-10-24 03:19'
labels:
  - h-aider-removal
milestone: m-24
dependencies:
  - CSP-700
priority: low
type: docs
ordinal: 777000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After the code is gone, operator-facing docs still describe aider support: the README harness lists and architecture diagram, `docs/operations.md` (per-repo scanning and the `harness_aider` theme example), `docs/provider-adapter-guide.md` (which points contributors at `aider.rs`), and `docs/library-api.md` (the adapter module list). Operators upgrading need a changelog entry that says aider support was removed and what happens to a leftover `harness_aider` setting.

Deferred aider task CSP-211 (native transcript parser) cannot land once the adapter is gone, so this workstream closes it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 README, `docs/operations.md`, `docs/provider-adapter-guide.md`, and `docs/library-api.md` no longer describe aider as supported; the adapter guide points at a remaining harness as its example
- [ ] #2 `CHANGELOG.md` records the removal under the next release, including what happens to a config that still sets `harness_aider`
- [ ] #3 Planning and history docs (`docs/implementation/`, earlier ADRs, `docs/backlog-legacy-ids.md`) stay unchanged, since they record history
- [ ] #4 CSP-211 is closed with a final summary saying the aider removal workstream superseded it
- [ ] #5 `git grep -i aider -- README.md docs/operations.md docs/provider-adapter-guide.md docs/library-api.md docs/tui-columns.md docs/design.md` returns nothing
- [ ] #6 `git diff --check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
