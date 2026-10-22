---
id: CSP-675
title: Rebuild repos by cloning them
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 15:06'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-659
  - CSP-672
  - CSP-665
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 733000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator chose repo clone as in scope, under the new envelope ADR's narrow `git clone` carve-out. It rebuilds a gone repo or primary checkout at its recorded path from its recorded remote.

- Scope: a clone executor running `git clone <remote> <path>` (first fetch remote by default, overridable in the slot), refusing an existing destination; act context only; a `docs/capabilities.toml` entry citing the new envelope ADR.
- Tests: executor tests for success, existing destination, and missing remote; static and traced capability tests pass with the new entry.
- Manual checks: delete a scratch clone and rebuild it from History.
- Blockers: the envelope ADR, the rebuild engine, and checkout recording tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A gone repo or primary checkout rebuilds by cloning its recorded remote to its recorded path
- [x] #2 Cloning refuses when the destination exists and never runs from a read context
- [x] #3 The capability manifest allows exactly this clone and cites the envelope ADR
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Repo rebuild clones the recorded remote to the recorded path with `git clone` (`rebuild/clone.rs`), refuses an existing destination, and only runs from the act context; the manifest allows exactly `clone * *` and writes under `$CLONE/**`, citing ADR 0143. Verified by `clone_tests` (including a real local clone) and the traced capability test, which clones for real.
<!-- SECTION:FINAL_SUMMARY:END -->
