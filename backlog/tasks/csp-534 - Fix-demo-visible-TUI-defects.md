---
id: CSP-534
title: Fix demo-visible TUI defects
status: Done
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: high
ordinal: 590000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Each sub-story was found on, and should be validated against,
  `tests/fixtures/showcase.json` with `conspectus tui --snapshot`. Land
  `CSP-540.01` alongside so the fixture reflects current discovery.
- **CSP-534.01** Render relative ages in the detail pane
- **CSP-534.02** Stop advertising `m choose` on ambiguous-mux rows
- **CSP-534.03** Make the help overlay readable
- **CSP-534.04** Keep related-row labels on their row
- **CSP-534.05** Fix the Mux view's session count
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-003`
