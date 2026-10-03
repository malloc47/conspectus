---
id: CSP-058
title: 'Document the `CONSPECTUS_DISABLE_FORGE`, `CONSPECTUS_DISABLE_TMUX`…'
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4-fu
milestone: m-9
dependencies: []
ordinal: 79000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Document the `CONSPECTUS_DISABLE_FORGE`, `CONSPECTUS_DISABLE_TMUX`, and `CONSPECTUS_*_STATE` env vars in `docs/design.md` or a new `docs/operations.md` so users discover them without grepping source.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `docs/operations.md` and linked it from
`docs/index.md`; the operations guide documents provider
toggles, harness state-root overrides, config precedence, current
CLI commands, and the no-cache-yet policy.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-FU-001`
