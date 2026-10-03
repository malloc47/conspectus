---
id: CSP-153
title: Pager auto-fit for table-style outputs
status: Done
assignee: []
created_date: '2026-05-18 22:51'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 130000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus table <ROWS>`, `conspectus columns <ROWS>`,
and `conspectus node show <id>` now pipe their output through a
pager when stdout is a TTY. Resolution order in
`pager_candidates()` (in `src/cli.rs`): `$PAGER` (split on
whitespace into program + args); otherwise `less` with
`LESS=FRX` defaults when `$LESS` is unset (`F`=quit if one
screen, `R`=raw control chars, `X`=no init/deinit) so short
tables print inline; otherwise `more`; otherwise direct print
when no pager spawns. Each affected subcommand gained
`--no-pager` (force off) and `--pager` (force on, clap-level
conflict with `--no-pager`). Non-TTY output stays direct by
default so existing pipe-based tests and `conspectus table
sessions | grep …` workflows are unchanged. `graph --format
json` and `declared list` deliberately stay direct (JSON is
machine-consumable; declared output is short-lived
tab-separated text — users can pipe through a pager manually
if needed). Four CLI integration tests cover the
`PAGER=cat --pager` round trip for `table` and `columns`,
`--no-pager` bypassing a pager that would otherwise fail
(`PAGER=false`), and the `--pager`/`--no-pager` clap conflict.
All 385 tests pass. `docs/operations.md` documents the new
behavior.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-013`
