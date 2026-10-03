---
id: CSP-154
title: Terminal color and styling for table output
status: Done
assignee: []
created_date: '2026-05-19 02:48'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 131000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0022 records the decision to use `anstyle` (already
transitive through clap) and the env-var precedence. The
renderer gained `RenderOptions.color` plus
`RenderOptions::with_color(bool)`; `agent_cell` /
`mux_cell` / `union_cell` / `pr_cell` / `fork_cell` keep their
`String` return shape because styling is applied by the
columnar/card emitters via a shared `push_styled` helper that
only injects ANSI when `color=true`. Width-aware truncation
runs on the unstyled text, so the ANSI envelope wraps already-
truncated cells and the budget math is unchanged. The initial
palette matches the ADR: bold headers and card-layout keys,
dim `—`/ID column, indicator-tier colors
(`LD`/`GD`=green, `SD`=cyan, `C`/`$`=dim), PR state colors,
DECLARED state colors, yellow on `draft`. `render_columns_listing`
and `render_node_show` accept a `color` flag and bold their keys
/ section headers; the table renderer keeps its richer palette.

Each affected CLI subcommand (`conspectus table <ROWS>`,
`conspectus columns <ROWS>`, `conspectus node show <id>`) gained
`--color {auto|always|never}`. The pure `resolve_color` helper
implements the ADR-0022 precedence (`--color=never|always`
short-circuit; `NO_COLOR` overrides `auto`; `CLICOLOR_FORCE`
forces on; `TERM=dumb` and `CLICOLOR=0` opt out; else `auto`
falls back to isatty). `resolve_color_from_env` wraps it with
the live process env.

Eight unit tests pin every resolver branch (never / always /
NO_COLOR / CLICOLOR_FORCE / TERM=dumb / CLICOLOR=0 / auto+TTY /
auto+pipe). Three renderer unit tests assert byte-identical
output when color is off, ANSI envelope presence on
`--color=on`, and the green PR-state color. Five CLI
integration tests cover `--color=always`/`never`/`auto`, the
NO_COLOR-vs-`--color=always` precedence (explicit user flag
wins), and color on `conspectus columns`. All 406 tests pass.
`docs/operations.md` documents the flag, the env precedence,
and the palette.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-014`
