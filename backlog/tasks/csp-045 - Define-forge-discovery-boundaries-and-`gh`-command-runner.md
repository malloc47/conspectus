---
id: CSP-045
title: Define forge discovery boundaries and `gh` command runner
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-042
ordinal: 43000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a `ForgeAdapter` trait and `ForgeDiscovery` provider under
  `src/discovery/forge/`, plus an injectable `gh` command runner that
  mirrors the existing `TmuxRunner` seam (real `SystemGh` that shells
  out, plus a `FakeGh` test runner). Outcomes are classified as
  `PullRequests(String)`, `Unavailable` (binary missing / unauthenticated),
  or `Failed { code, message }` so tests can drive each path
  deterministically. Record an ADR if the choice to delegate to `gh`
  (rather than calling the GitHub REST API directly) needs to outlive
  the implementation plan.
- Tests: unit tests for missing `gh` binary, unauthenticated runs,
  command failures, empty output, and stable diagnostic strings.
- Manual checks: confirm no test requires a real `gh` install or
  network call; inspect the module layout for ADR 0007 alignment and
  verify forge discovery performs no rendering.
- Blockers: `CSP-042`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `discovery::forge` with a `ForgeAdapter` trait, a
`ForgeDiscovery` coordinator, and a `GhRunner` seam (`SystemGh`
shells out to `gh pr list --json`, `FakeGh` returns pre-canned
outcomes and records the spawn cwd). `GhOutcome` classifies runs
as `PullRequests`/`Unavailable`/`Failed`; `GhUnavailableReason`
covers binary-missing, unauthenticated, and not-a-repo cases.
ADR 0011 records the decision to delegate to `gh` rather than
adding an HTTP client. Nine new unit tests cover each path; no
test requires a real `gh` install or network.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-001`
