---
id: CSP-863
title: Trim debug info and optimize dependencies in the dev profile
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 03:54'
updated_date: '2026-11-13 22:23'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Measure the baseline on this machine (body-only edit to copy_to_clipboard): incremental test build, nextest run, test binary sizes, then a whole `just check`, with load and free memory.
2. Add `[profile.dev] debug = "line-tables-only"` and `[profile.dev.package."*"] opt-level = 1` to the root Cargo.toml (the test profile inherits dev; workspace members stay at opt-level 0).
3. Verify a panicking test still reports file:line for the panic and its backtrace frames (RUST_BACKTRACE=1 on a scratch failing test).
4. Write the ADR (profile, measured effect, alternatives: split-debuginfo, debug off for dependencies only, a faster linker, a higher opt-level for dependencies; whether the exported framework manifest carries the profile).
5. Re-measure after (same edit), run the full suite, checks.sh, `just tuikit-features`, `just separation-rehearsal --self-test`, and commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Baseline 2026-11-13 on this machine (8 vCPU, 15 GiB, swap 4 GiB and full), followups at b3c9ec25, default profile, the same body-only edit (`std::hint::black_box(0_u8);` at the top of `copy_to_clipboard`):
- Incremental test build (`cargo test --workspace --all-targets --all-features --no-run`): 40.8 s wall (92 s user + 37 s sys); load 0.8, 10.1 GB available. 48 binaries relinked, 8.0 GB in all; `html_snapshots` 258 MB (`.debug_*` about 240 MB of it). The linker is already LLD (the binaries' `.comment` says `Linker: LLD 23.1.1`).
- nextest run: 74.5 s wall (178 s user + 106 s sys), 5,681 passed, 1 skipped; load 6.1.
- Whole `just check`: 427 s (fmt 4, clippy 23, cargo test 153, nextest 71, tuikit-features 164 of which the two test legs 116, doc 12); load 4.6, 10.2 GB available. A first attempt failed after 77 s on the known `Text file busy` flake in `discovery::forge` under `cargo test`.

Steady-state baseline `just check` (second run, the framework matrix's feature sets cached): 330 s (fmt 5, clippy 26, cargo test 160 with the build, nextest 74, tuikit-features 55, doc 11); load 8.2 at start, 9.9 GB available, swap full.

Alternatives, each in a fresh scratch target dir (`target/exp`, deleted after) on the baseline manifest, same edit; load 6 to 11 throughout, so treat nextest as about ±10 s:
- `split-debuginfo = "unpacked"` (full debug info in `.dwo` files): cold 155 s; incremental test build 29.5 s (65 s user + 24 s sys); 48 binaries 2.3 GB, `html_snapshots` 74 MB; nextest 72.8 s; fresh target 6.9 GB. Backtraces keep file:line (checked on a probe crate).
- debug info off for dependencies only (`profile.dev.package."*".debug = false`): cold 150 s; incremental 30.7 s; binaries 5.9 GB, `html_snapshots` 196 MB; nextest 66.4 s; fresh target 11 GB.
- line tables + dependencies at `opt-level = 2`: cold 204 s; incremental 25.5 s; binaries 3.0 GB, `html_snapshots` 97 MB; nextest 117 s at load 7 to 11 with 209 s of sys time (contention; not a fair reading); fresh target 6.8 GB.
<!-- SECTION:NOTES:END -->
