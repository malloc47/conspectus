---
id: CSP-878
title: Make the forge concurrency test independent of thread timing under load
status: To Do
assignee: []
created_date: '2026-11-13 23:44'
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
- [ ] #1 The test asserts the concurrency limit without depending on a fixed sleep overlapping, and passes repeatedly under load (for example 20 runs while another build runs)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
