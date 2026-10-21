---
id: CSP-651
title: Make conspectus serve honour configured scan roots
status: To Do
assignee:
  - '@claude'
created_date: '2026-10-21 19:39'
labels:
  - server
  - config
dependencies: []
references:
  - src/cli/lifecycle.rs
  - src/cwd.rs
  - docs/adr/0111-launch-directory-is-optional.md
priority: medium
type: bug
ordinal: 709000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0111 says survey commands resolve scan roots as `--scan-root` flags, then configured roots, then the launch directory. `conspectus tui` follows that order using `[tui] scan_roots`, but `conspectus serve` passes an empty configured list (`ScanRoots::resolve(self.scan_roots, &[], cwd)` in `src/cli/lifecycle.rs`), so it only ever sees flags or its own launch directory.

When the daemon is running the TUI reads its snapshot instead of discovering itself (`discover_and_resolve_sourced` in `src/tui/runtime.rs`), so a configured `[tui] scan_roots` silently stops applying whenever `serve` is up, and the daemon and in-process snapshots disagree about what was discovered. This was found while triaging CSP-650 and the operator's request for workspace-grouped muxes: configuring scan roots was the obvious stopgap and it only worked without the daemon.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With `[tui] scan_roots` configured and no `--scan-root` flag, `conspectus serve` discovers from the configured roots, and its startup line lists them
- [ ] #2 `--scan-root` flags still take precedence over configured roots, and with neither the launch directory is used as before (ADR 0111)
- [ ] #3 Tests cover the serve root resolution order
- [ ] #4 docs/operations.md states that serve reads `[tui] scan_roots`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
