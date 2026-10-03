---
id: CSP-080
title: >-
  Refresh top-level docs to position Conspectus as the cross-workspace
  observability surface
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-076
  - CSP-077
  - CSP-079
ordinal: 75000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `README.md` so it no longer reads "currently in
  design"; describe what the CLI does today and link the
  feature summary, ADR index, and migration guide. Update
  `docs/index.md` if the table of contents shifted. Update the
  "Migration Plan" section of `docs/design.md` to mark items 1–5
  complete and reference Phase 6's ADRs for items 6–7.
- Tests: docs-only; `git diff --check`.
- Manual checks: open the rendered Markdown and confirm the
  framing matches the post-Phase-5 reality.
- Blockers: `CSP-076`, `CSP-077`, `CSP-079`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Refreshed `README.md` so it describes the implemented CLI
and library instead of a design-only project, links the feature
summary, ADRs, operations, library API, and Atelier migration guide,
and updates development checks. Updated `docs/design.md` to mark
migration-plan items 1-5 complete, identify item 6 as tracked by
the Conspectus and Atelier Phase 6 coordination docs, and reference
ADRs 0015 and 0016 for the API/distribution decisions around items
6-7.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-008`
