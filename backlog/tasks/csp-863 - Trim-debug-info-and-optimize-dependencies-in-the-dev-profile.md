---
id: CSP-863
title: Trim debug info and optimize dependencies in the dev profile
status: To Do
assignee: []
created_date: '2026-11-13 03:54'
labels:
  - h-devloop
milestone: m-11
dependencies: []
references:
  - Cargo.toml
  - docs/adr/0110-pinned-rust-toolchain.md
priority: high
type: chore
ordinal: 893000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every edit relinks about 20 integration-test binaries of about 220 MB each, and 92% of each is debug info (`html_snapshots`: 199.5 MB of 216 MB in `.debug_*` sections). Separately, the randomized sweep tests and `src/tui/consistency_tests` run their dependency code (ratatui, unicode-width) unoptimized, which is where most test CPU goes.

Measured 2026-11-12 on the 8-vCPU dev VM. The edit was a body-only change (a `std::hint::black_box(0_u8);` statement at the top of `copy_to_clipboard` in `src/tui/clipboard.rs`). The variants were built with `CARGO_PROFILE_DEV_DEBUG` and `--config` overrides in a scratch target dir:

| Profile | Test build | Link CPU | Test binary | nextest run (test CPU) | Cold build |
|---|---|---|---|---|---|
| default | 39.3 s | 172 s | 224 MB | 75-85 s (549-639 s) | - |
| `debug = "line-tables-only"` | 29.0 s | 83 s | 76 MB | 70 s (485 s) | 130 s |
| + `opt-level = 1` for non-workspace packages | - | - | - | 56 s (388 s) | 222 s |

With optimized dependencies, the host `consistency_tests` dropped from 115 s to 48 s of CPU. Load on the machine varied between 5 and 17 during these runs, so treat the nextest numbers as about ±10 s.

Costs: a debugger can no longer show local variables, though backtraces keep file:line. A cold build, paid once per worktree and again after a dependency or toolchain bump, takes about 90 s longer. No check is dropped.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The dev profile, and through it the test profile, emits line tables only for debug info, and builds non-workspace packages at `opt-level = 1`; workspace members stay unoptimized
- [ ] #2 A panicking test still reports the file and line of the panic and of its backtrace frames
- [ ] #3 An ADR records the profile, the measured effect, and the alternatives weighed (at least `split-debuginfo`, debug info off for dependencies only, a faster linker, and a higher `opt-level` for dependencies)
- [ ] #4 The ADR states whether the exported framework manifest carries the profile, and `just separation-rehearsal --self-test` passes
- [ ] #5 The final summary records before and after timings on one machine for the same one-line host edit: test build, nextest run, and test binary size
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
