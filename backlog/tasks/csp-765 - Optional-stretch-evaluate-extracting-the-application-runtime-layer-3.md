---
id: CSP-765
title: 'Optional stretch: evaluate extracting the application runtime (layer 3)'
status: To Do
assignee: []
created_date: '2026-10-27 20:28'
labels:
  - tui-framework
  - tuifw-close
milestone: m-25
dependencies:
  - CSP-729
  - CSP-731
  - CSP-732
  - CSP-757
documentation:
  - docs/tui-architecture-review.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: low
type: spike
ordinal: 851000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150 leaves the application runtime in Conspectus: the MVU event loop, the effect lifecycle, undo, and terminal suspend and resume for subprocess hand-off. Once layers 1 and 2 are in the framework, it is worth knowing how much of the runtime is generic and whether extracting it would pay for itself. `docs/tui-architecture-review.md` records what changed since the review (effects as data, the modal stack, the declarative keys), and the interaction-conventions inventory marks the effect lifecycle and undo classifier as "layer 3 or domain".

Scope: Measure and write up: which runtime code is generic (the poll and render loop, toast ticking, the exhaustive effect-classifier pattern, terminal suspend and resume) and which knows the graph model (effects, undo, the reducer's data); what a `Runtime<M, Fx>` seam would look like; the cost in stories; and the benefit for a second consumer. Recommend go, no-go, or later, and if go, outline the stories.

TUI changes: none; this is an evaluation.

Tests and checks: The write-up lists the files and line counts that fall on each side. It is reviewed against the framework's layer 2 contracts.

Blockers: Optional: only worth doing if the extraction work has gone smoothly and the operator wants a second consumer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A write-up classifies the runtime code as generic or domain, with files and line counts
- [ ] #2 It sketches the runtime seam and estimates the stories
- [ ] #3 It recommends go, no-go, or later, with reasons
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
