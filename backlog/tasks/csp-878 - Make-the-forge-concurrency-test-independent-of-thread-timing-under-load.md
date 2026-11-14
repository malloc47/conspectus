---
id: CSP-878
title: Make the forge concurrency test independent of thread timing under load
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 23:44'
updated_date: '2026-11-14 02:21'
labels:
  - h-devloop
milestone: m-11
dependencies: []
priority: low
type: bug
ordinal: 955000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`discovery::forge::github::tests::fetches_run_at_most_the_configured_number_of_gh_calls_at_once` (src/discovery/forge/github_tests.rs) expects twelve fake `gh` calls in flight at once with the limit at 64: each call sleeps 30 ms, and the test asserts the observed peak equals 12. Under load the threads do not all overlap within 30 ms. On 2026-11-13 a whole `just check` at load 4 to 6 failed with `limit 64: left: 10, right: 12`; the test passed alone three times right after. It fails a local check at random and costs a rerun, and CI can hit it as well.

Scope: make the assertion hold however the threads are scheduled, for example by having the fake calls wait on a barrier for the expected peak (with a timeout) instead of sleeping, or by asserting the peak never exceeds the limit and reaches it when the calls are held open.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The test asserts the concurrency limit without depending on a fixed sleep overlapping, and passes repeatedly under load (for example 20 runs while another build runs)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Replace the 30 ms sleep in the test's fake `gh` runner (src/discovery/forge/github_tests.rs) with a rendezvous: each call stays open (Mutex + Condvar) until the expected peak of calls is in flight at once, with a 10 s deadline so a limiter that lets fewer through fails instead of hanging, then lingers 20 ms so calls an over-permissive limiter let through can still raise the peak. The assertion (peak equals min(limit, 12)) is unchanged.
2. Prove it: run the test alone, one run at a time, many times while 2x CPUs of busy loops load the machine, the old version first (to show the load reproduces the flake) and then the new one; report the counts.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Loop counts (the test binary run alone, one run at a time, `--exact`): 48 busy loops on 8 CPUs (load ~23-30): old 42 passed / 18 failed (all `limit 64`), new 60/0. 16 busy loops (load ~9-21): old 59/1, new 60/0. During a `cargo build --release` (load ~10-13): old 40/0, new 40/0. Mutation checks: chunking by the job count instead of the limit fails at limit 1 (peak 12); chunking by half the limit fails at limit 3 (peak 1) after the shared 10 s deadline. No docs change: test only.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The fake runner of `fetches_run_at_most_the_configured_number_of_gh_calls_at_once` (src/discovery/forge/github_tests.rs) no longer sleeps 30 ms hoping the calls overlap: each call stays open (Mutex + Condvar) until the expected peak, min(limit, 12), is in flight at once, under a 10 s deadline shared by the runner's calls, then 20 ms longer so an over-permissive limiter can still show. The assertions are unchanged; the test takes about 0.35 s.

Evidence: 60 runs under 48 busy loops on 8 CPUs, old 18 failures, new 0; 60 under 16 busy loops, old 1, new 0; 40 during a release build, both 0. A limiter that ignores the limit still fails it (limit 1 peaks at 12), and one that lets too few through fails within 10.5 s. `checks.sh` and `just check` (5705 passed) green. No docs change (test only).
<!-- SECTION:FINAL_SUMMARY:END -->
