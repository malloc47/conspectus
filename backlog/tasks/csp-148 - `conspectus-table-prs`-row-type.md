---
id: CSP-148
title: '`conspectus table prs` row-type'
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 125000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Projection::Pr` joins the row-type enum; the registry
`PRS_COLUMNS` declares `id`, `pr`, `state`, `draft`, `branch`,
`repo`, `updated`, and `attached`, with the default set
`id, pr, state, branch, attached`. `PrRowCtx` + `pr_cell` extract
cells: the `branch` column walks `BranchHasForgePr` and strips
the `refs/heads/` prefix; `attached` finds checkouts via
`CheckedOutBranch` candidates and joins agent sessions whose
`cwd` matches the checkout root; `updated` formats
`updated_epoch` via the new `format_relative_age` helper
(`12s`, `5m`, `2h`, `3d`, `4w`). Config grew `[table.prs]`. CLI
subcommand `conspectus table prs` honors the existing `--wide`,
`--width`, `--layout`, `--scan-root`, and `--columns` flags.
Eight new unit tests cover `format_relative_age`,
`strip_branch_prefix`, the default header, the `attached`
discovery via the BranchHasForgePr → CheckedOutBranch path,
optional-column rendering, and the empty-snapshot case. Two
CLI integration tests cover the default projection header and
the optional-columns flag. All 365 tests pass; existing
snapshots stay byte-for-byte stable.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-008`
