---
id: CSP-865
title: Run the workspace tests once in `just check` and keep `cargo test` in CI
status: To Do
assignee: []
created_date: '2026-11-13 03:55'
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
- [ ] #1 `just check` runs the workspace tests once, through nextest, and `just test` remains available as its own recipe
- [ ] #2 CI still runs both `cargo test` and nextest on every push and pull request
- [ ] #3 The pre-main invariants in AGENTS.md, the dev-shell banner, and any README or docs page that lists the local checks name nextest as the local run and `cargo test` as a CI run
- [ ] #4 An ADR records the change and names the part of ADR 0007 it supersedes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
