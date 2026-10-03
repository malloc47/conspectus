---
id: CSP-367
title: Pin CLI command tree skeleton
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-361
ordinal: 300000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `conspectus pin {create,list,show,rename,rm,launch,
  attach,bind,rebind,adopt}` subcommand structure with flag
  surface from ADR 0057. Validation only — write commands stub
  `bail!("not yet implemented")`. Read commands wire up in
  CSP-368. `--help` text matches ADR. `--mux-socket` flag
  accepts a tmux socket name (the equivalent of `tmux -L`); the
  TOML key it writes is `mux.socket_name`.
- Tests: CLI smoke tests for `--help`, invalid flag combinations,
  and missing required arguments per subcommand.
- Blockers: `CSP-361`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus pin` exposes the v1 create/list/show/rename/
rm/launch/attach/bind/rebind/adopt command tree.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-007`
