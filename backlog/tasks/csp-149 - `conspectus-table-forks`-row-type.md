---
id: CSP-149
title: '`conspectus table forks` row-type'
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 126000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Projection::Fork` joins the row-type enum; the registry
`FORKS_COLUMNS` declares `id`, `fork`, `provider`, `scope`,
`parent`, `children`, and `capabilities`, with the default set
`id, fork, provider, parent, children`. `ForkRowCtx` +
`fork_cell` extract cells: `fork` renders `{provider}:{name}` or
falls back to `provider_source_key` when no name is set;
`parent` follows the fork's preferred `ParentSession` candidate
to a short session id (unresolved parents prefixed with `?`);
`children` counts `ChildSession` candidates from the fork that
target agent-session endpoints (resolved or unresolved). Forks
are now indexed on `SnapshotView` alongside the other typed
node maps. Config grew `[table.forks]`. CLI subcommand
`conspectus table forks` honors the existing `--wide`,
`--width`, `--layout`, `--scan-root`, and `--columns` flags.
Four unit tests cover the default header, fork-label fallback,
parent/children extraction, and capabilities-as-optional-column
rendering. One CLI integration test exercises the default
header. All 370 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-009`
