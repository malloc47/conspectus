---
id: CSP-867
title: 'Reduce contention for memory, CPU, and disk on the dev machine'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 03:55'
updated_date: '2026-11-13 23:45'
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
- [x] #4 The `target/` directories of merged or abandoned worktrees are cleaned
- [ ] #5 A full `just check` run with the usual set of agent sessions open completes with swap headroom and no OOM kill in the kernel log
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Draft the scanner-exclusion request for the operator to send (task notes); AC #1 and #2 wait on its outcome.
2. Remove the target/ directories of the two merged worktrees (tui-framework, live-state), nothing else in them.
3. Remove provably finished /tmp scratch: /tmp/tuifw-baseline, /tmp/tuifw-tasks, empty nix-develop-* directories older than a day, ls-tmux.* and csp-fu-tmux.* directories older than a day with no live socket. List what is left, with sizes.
4. Accumulation: CSP-866's scripts/dev-env.sh ends the largest source (a nix-develop-* and a nix-shell.* directory per wrapped command); propose the remaining steps, which live outside the repository, to the operator.
5. Run a full `just check` with the usual sessions open; record free memory, swap, and kernel-log OOM lines before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC #1, draft for the operator to send to the machine administrators (not sent; nobody was contacted):

Subject: Request to exclude Rust build output directories from endpoint scanning on chvld-jwaggone1

Hello,

On my development VM chvld-jwaggone1 (8 vCPU, 16 GB, user jwaggoner), the endpoint security agent (SentinelOne: s1-agent and s1-scanner) takes a large share of the CPU my builds use. During one 12-minute build-and-test run on 2026-11-12 it averaged 1.18 cores, about twelve times its idle rate. Each such run writes about 6 GB of newly linked test executables and starts about 5,000 short-lived test processes, and both appear to be scanned.

I would like to ask for these directories to be excluded from real-time (on-write) file scanning and, if your policy allows it, from the monitoring of processes started from them:

  /home/jwaggoner/src/oss/conspectus/target/
  /home/jwaggoner/src/oss/conspectus/.claude/worktrees/*/target/

If one directory is easier to approve, a single root such as /home/jwaggoner/.cache/cargo-target/ would also work; I would point every checkout's builds there.

These directories hold only compiler output (object files, Rust libraries, and test executables) that the Rust toolchain builds locally from source I am working on. Nothing in them is downloaded or installed, and all of it can be deleted and rebuilt. I am not asking to exclude the source trees, my home directory, or the Nix store.

Please let me know what is granted, or what you would suggest instead.

Thank you,
Jarrell Waggoner

AC #1 and AC #2 wait on the operator: send the request and record the outcome here. If per-worktree paths are granted, nothing changes in the repository. If only a single root is granted, worktrees need `CARGO_TARGET_DIR=<root>/<worktree name>`; the natural place is scripts/dev-env.sh (CSP-866), and that is a repository convention, so it gets an ADR then.

AC #4 (done): removed only the `target/` directories of the two merged worktrees, `.claude/worktrees/tui-framework/target` (22 GB) and `.claude/worktrees/live-state/target` (1.6 GB); both branches are ancestors of `main`, no build ran there (two idle sessions have their working directory in tui-framework and will rebuild on their next build). Disk: 38 GB free (87%) -> 62 GB free (79%). Left for the operator, as instructed: `launch-spinner/target` 9.9 GB and `pin-resume-preserve-argv/target` 5.7 GB (both branches are also merged into main, last commits 4 weeks ago), and the main checkout's `target/` 47 GB.

AC #3 (removal done, accumulation waits on the operator): removed `/tmp/tuifw-baseline` (246 MB), `/tmp/tuifw-tasks` (0.3 MB), 7,880 empty `nix-develop-*` directories older than a day, and 223 `ls-tmux.*` directories older than a day (all empty; no socket under any `ls-tmux.*` or `csp-fu-tmux.*`). `/tmp` 2.8 GB -> 2.5 GB used; swap free 8 MB -> 254 MB right after. Left, with sizes: 8,699 `nix-shell.*` (448 MB, 7,701 empty; not on the list I may remove), 815 empty `nix-develop-*` and 112 `ls-tmux.*` younger than a day, 19 `csp-fu-tmux.*` from today, and other sessions' directories I cannot attribute to a finished session: dc-scope-src 517 MB, tuifw-716 470 MB, pi-research 292 MB, dc-dryrun 283 MB, nono-research 108 MB, pi-survey 63 MB, maj-pi-safety 48 MB, omniwm-src.ucHF 41 MB, csp-fu 38 MB, drw-who-34098 29 MB, fork-bundles-IeJZ 24 MB, dp-base 19 MB, fixb 18 MB, dp-p3 18 MB, cspwork 18 MB, node-compile-cache 14 MB, and smaller ones.
Accumulation, smallest changes: (1) in the repository, CSP-866's `scripts/dev-env.sh` ends the largest source, one `nix-develop-*` and one `nix-shell.*` per `nix develop --command` (about 17,000 in three weeks), once agents use it (the AGENTS.md patch); the repository's own tools already clean up (tests use self-deleting temp dirs, `just scratch` removes its home, the separation rehearsal works under `target/`), so no repository convention or ADR is needed. (2) Operator steps, outside the repository: apply the AGENTS.md patch; make the helper scripts that create a private `TMUX_TMPDIR` with mktemp (`/tmp/csp-fu/run-tests.sh`, `run-some.sh`, and the live-state ones that made `ls-tmux.*`) remove it on exit, e.g. `trap 'rm -rf "$TMUX_TMPDIR"' EXIT` after the mktemp line; remove the old `nix-shell.*` directories, e.g. `find /tmp -maxdepth 1 -name 'nix-shell.*' -user "$USER" -mtime +1 -empty -delete`, then review the non-empty ones; review the other sessions' directories above; optionally ask agents (in the global instructions) to keep large scratch on disk, for example under the worktree's `target/`, and delete it at the end.

AC #5 (waits on the operator): a whole `just check` after the one-line edit, with the usual sessions open (load 4.5 to 7.9), passed in 186 s; no OOM line in the kernel log for the window (`journalctl -k`; `dmesg` itself fails with "dmesg: read kernel buffer failed: Operation not permitted"); available memory stayed at or above 7.7 GB; memory pressure (/proc/pressure/memory) near zero. But swap stayed full: 33 MB free at the start, 0 MB at the low point. The swap holds idle processes' pages (4 GB swap file, many idle agent sessions, the security agent at 0.9 GB RSS), not this build's. Headroom needs an operator or administrator step: close idle sessions, enlarge `/swap.img`, or cycle swap (`sudo swapoff -a && sudo swapon -a` while enough RAM is free). The last OOM kill in the kernel log is 2026-11-11 18:16 (emacs). An earlier attempt of the same run failed on a timing-dependent forge test (passed alone three times; filed as CSP-878).
<!-- SECTION:NOTES:END -->
