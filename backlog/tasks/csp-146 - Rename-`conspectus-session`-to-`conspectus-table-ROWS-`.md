---
id: CSP-146
title: Rename `conspectus session` to `conspectus table <ROWS>`
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 123000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0021 records the rename. The CLI grew a `table`
subcommand tree with `Sessions`, `Mux`, and `Union` subcommands;
each takes the existing `--wide`, `--width`, `--layout`, and
`--scan-root` flags via shared `TableRowsArgs`. The old
`session` subcommand and `--projection` flag are gone with no
alias (per CLAUDE.md). Config migrated from `[session].projection`
to `[table.<rows>]` per-row-type subsections — empty for CSP-146
but reserved for the column registry in CSP-147. A legacy
`[session]` section in user config now produces a stderr
diagnostic pointing at the new schema; the run still proceeds.
`Projection::parse` accepts both `agent` (legacy) and `sessions`
(new) for the agent-projection row-type. All 340 tests pass,
including the existing insta snapshots: the rename is
CLI-and-config-only, the renderer internals
(`build_*_rows`, `RenderOptions`, `node_short_id`) are
unchanged. `docs/operations.md` documents the new shape.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-006`
