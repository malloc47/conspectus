---
id: CSP-865
title: Run the workspace tests once in `just check` and keep `cargo test` in CI
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 03:55'
updated_date: '2026-11-13 23:06'
labels:
  - h-devloop
milestone: m-11
dependencies: []
references:
  - justfile
  - .github/workflows/ci.yml
  - docs/adr/0007-rust-development-approach.md
  - docs/adr/0099-caller-owned-discovery-caches.md
priority: medium
type: chore
ordinal: 895000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`just check` runs `cargo test --workspace --all-targets --all-features` and then `cargo nextest run` with the same flags, so the same 4,920 tests run twice. Measured 2026-11-12, that was 101 s for `cargo test` and 75-85 s for nextest. `cargo test` also runs its binaries one after another, so the 33 s capability trace (`tests/capabilities.rs`) holds up everything behind it.

ADR 0007 listed both runners as baseline checks. ADR 0099 later removed the process-global caches whose sharing made the two runners behave differently, and concluded that "test isolation no longer depends on process isolation."

The operator accepted the trade-off on 2026-11-12: a test that fails only when it shares a process with other tests will now be caught by CI before `main`, not by the local check.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `just check` runs the workspace tests once, through nextest, and `just test` remains available as its own recipe
- [x] #2 CI still runs both `cargo test` and nextest on every push and pull request
- [ ] #3 The pre-main invariants in AGENTS.md, the dev-shell banner, and any README or docs page that lists the local checks name nextest as the local run and `cargo test` as a CI run
- [x] #4 An ADR records the change and names the part of ADR 0007 it supersedes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `justfile`: drop `test` from `check` (the tests run once, through `just nextest`); keep the `test` recipe, with a comment saying it is CI's shared-process run.
2. CI: keep both the `cargo test` and the nextest steps; add a comment on the `cargo test` step saying why it stays.
3. ADR 0224: the change, the measured cost, the trade-off the operator accepted, and the part of ADR 0007 it supersedes; mark ADR 0007 amended in its status and in the index.
4. Docs: the dev-shell banner and the README development section name nextest as the local run and `cargo test` as a CI run. AGENTS.md's pre-main invariants: the edit is prepared as a patch for the operator, since an agent may not change its own instruction file on another agent's request.
5. Measure the whole `just check` before and after, run the full suite and the checks, commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed: `check` no longer lists `test` (the tests run once, through `just nextest`); `just test` stays with a comment saying it is CI's shared-process run; CI keeps both steps, with a comment on the `cargo test` step; ADR 0224 records it and amends ADR 0007 (its status and the index say so); the dev-shell banner and the README development section name nextest as the local run and `cargo test` as a CI run (`just test`).
Measured, same machine, same one-line edit, with CSP-863's profile in place: whole `just check` 306 s (cargo test step 148 s, nextest 64 s) -> 233 s (nextest step 119 s with the build), load 6 rising to 20 during the run. Neither runner runs doctests with `--all-targets`, so nothing is lost locally; the framework's doctests run in `just tuikit-features`.
Checks: the whole `just check` passed; run-tests.sh (cargo test) 5,681 passed, 0 failed; checks.sh clean.
WAITS ON THE OPERATOR (AC #3, DoD #2): AGENTS.md is the agent's own instruction file (CLAUDE.md links to it), and an agent may not change it on another agent's request, so its pre-main invariant is not edited. Proposed replacement for the "Relevant tests pass" bullet:
- Relevant tests pass: run
  `cargo nextest run --workspace --all-targets --all-features` locally
  (`just check` runs it). CI also runs
  `cargo test --workspace --all-targets --all-features`, which catches a
  test that fails only when it shares a process with others; `just test`
  runs it locally.
<!-- SECTION:NOTES:END -->
