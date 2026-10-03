---
id: CSP-465
title: Remove the argv-sniffing fake-mtime test backdoor
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 96000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-04 (commit `fd1bf39`). Deleted the
  `is_cargo_test_process` argv-sniff + the
  `#[cfg(not(test))]` / `#[cfg(test)]` `file_modified_epoch`
  variants from all three affected harness adapters
  (aider, claude_code, codex). Fixture writers in
  `discovery::harness::fixtures` now stamp mtimes
  deterministically via `File::set_modified` and a new
  `stamp_fixture_mtime(path)` helper reading the fixed
  `FIXTURE_MTIME_EPOCH = 1_700_000_000` constant. No new
  dependency. Production `file_modified_epoch` collapses
  to a single-impl real `fs::metadata` read. Net −51 / +42
  across 4 files. `grep is_cargo_test_process src/`
  returns only the rationale comment.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-004`
