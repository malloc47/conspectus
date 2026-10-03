---
id: CSP-538
title: True up backlog checkboxes
status: Done
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: high
ordinal: 605000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope:
  - Tick, with an outcome line: `CSP-501` (`b84aa05`),
    `CSP-502` (`93406b5`), `CSP-525` (`084967f`),
    `CSP-528` (`c6905bb`), `CSP-402` (`4c63044`), `CSP-509`
    (CLI `H-WT-004a` plus TUI `CSP-509.02`), and the `CSP-506` epic (`CSP-507`, `CSP-508`, `CSP-509`, `CSP-521`, `CSP-522`, `CSP-523`, `CSP-524`
    landed). Also tick `CSP-143`: the daemon snapshot read path landed as
    `try_daemon_snapshot` (`src/cli/mod.rs:209`), though the story text
    still says `conspectus session`. Tick `CSP-163` too: row builders for
    all five row types live in `src/tui/rows/`.
  - Tick and move the remainder: `CSP-166` (v1 landed in `bc7a1b0`; the
    empty/loading/error frame matrix continues as `CSP-180`).
  - Verify with the operator: `CSP-461` (WIP `719d77a`, then
    `8870e82`, `fc55193`, `88450d8`, `0524fdd`).
  - Annotate and leave open: `CSP-170` (`S` resumes in a new terminal;
    resuming into a mux is still open) and `CSP-175` (see `CSP-534.02`).
- Tests: docs-only; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ticked the eleven landed items with outcome notes (including
`CSP-461`, confirmed by the operator) and annotated `CSP-170`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `REL-007`
