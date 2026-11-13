---
id: CSP-867
title: 'Reduce contention for memory, CPU, and disk on the dev machine'
status: To Do
assignee: []
created_date: '2026-11-13 03:55'
labels:
  - h-devloop
milestone: m-11
dependencies: []
priority: high
type: chore
ordinal: 897000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The machine itself slows every build and test run. Measured 2026-11-12 on the 8-vCPU, 16 GB dev VM:

- **Memory:** swap was 100% full, and the kernel OOM killer fired on 2026-11-11 and 2026-11-12. A workspace test build needs 2.7-2.9 GB of anonymous memory, plus up to 5 GB of page cache. The operator then closed a number of idle agent sessions, which left 8 GB available and about 0.5 GB of swap free.
- **`/tmp`:** it is RAM-backed (tmpfs) and held 2.6 GB of scratch directories left by earlier agent sessions. Those pages end up in swap.
- **CPU:** the endpoint security scanner averaged 1.18 cores over a 12-minute build-and-test run, about 12 times its idle rate. That run linked about 6 GB of test binaries and started about 5,000 test processes per nextest run.
- **Disk:** the five worktrees' `target/` directories held about 104 GB, with the disk 84% full.

None of this lives in the repository, but this machine is where the agent loop runs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An exclusion of the cargo target directories (or of one target root that worktrees can share) from endpoint scanning is requested from the machine administrators, and the outcome is recorded in the task notes
- [ ] #2 If an excluded location is granted, how a worktree points its target directory there is decided, and recorded in an ADR if it becomes a repository convention
- [ ] #3 Scratch directories that finished sessions left in `/tmp` are removed, and agent scratch no longer accumulates in RAM-backed `/tmp` across sessions
- [ ] #4 The `target/` directories of merged or abandoned worktrees are cleaned
- [ ] #5 A full `just check` run with the usual set of agent sessions open completes with swap headroom and no OOM kill in the kernel log
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
