---
id: CSP-551
title: Add contributor onboarding
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
updated_date: '2026-10-31 08:16'
labels:
  - rel
milestone: m-20
dependencies:
  - CSP-533.02
priority: low
ordinal: 622000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `CONTRIBUTING.md` explaining the ADR + backlog workflow for
  outside contributors (stable IDs, when an ADR is required,
  `just check`, snapshot mode), plus issue/PR templates if the repo
  accepts outside contributions.
- Blockers: `CSP-533.02`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-020`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): onboarding now has two halves. The framework crate has its own contribution rules (`crates/tuikit/docs/contributing.md`: the independence guards, the control page template, how to add a control or a dependency), its house rules (`crates/tuikit/docs/house-rules.md`), and its own decision log (`crates/tuikit/docs/decisions/`, canonical once the crate leaves the repository). `CONTRIBUTING.md` should point there for TUI work and name the extra checks (`just tuikit-features`, `just separation-rehearsal --self-test`).
<!-- SECTION:NOTES:END -->
