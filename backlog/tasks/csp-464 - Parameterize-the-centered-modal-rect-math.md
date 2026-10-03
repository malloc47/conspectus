---
id: CSP-464
title: Parameterize the centered-modal rect math
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 95000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-04 (commit `9aee2bf`). New
  `popup_frame::centered_rect(area, width, height) -> Rect`
  absorbs the 4-line centering arithmetic all six overlays
  duplicated. Each overlay retains its own width / height
  policy locally (the caps and height derivations
  legitimately differ). Deferred: collapsing into
  `tui-popup`'s percentage-based centering — mismatched
  against Conspectus's absolute width caps (78, 60, 50, …)
  and would produce different observable widths at typical
  terminal sizes. Net −55 / +20 across 7 files.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-003`
