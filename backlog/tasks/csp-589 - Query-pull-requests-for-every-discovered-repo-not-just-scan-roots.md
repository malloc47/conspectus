---
id: CSP-589
title: 'Query pull requests for every discovered repo, not just scan roots'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-12 23:35'
updated_date: '2026-10-12 23:54'
labels:
  - p4-fu
milestone: m-9
dependencies: []
priority: high
ordinal: 638000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: forge discovery probes only the discovery roots themselves, so a root like `~/src` yields no PRs even though git discovery finds the repos under it through session and mux working directories. In practice only the launch repo's PRs appear.
- Scope: run forge discovery after the working-directory git pass, over every Repo node the graph found, reading remotes and local branches from the graph rather than re-probing roots. Bound concurrent `gh` calls.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 PRs are discovered for every repo in the graph with a GitHub remote, including repos found only through session or mux working directories
- [x] #2 Concurrent `gh` calls are bounded, and the forge cache still skips refetching within its interval
- [x] #3 Tests cover a repo reached only through a session working directory
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Run forge after the working-directory pass over the graph's repos (ADR 0115); re-probe each through the cached git probe; bounded scoped-thread calls; cache keyed by repo set.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Forge left the keyed provider list and runs after `merge_with_prior`, so repos from a fresh-skipped git class (in the prior snapshot) are covered too; a fresh `github` key still skips it.
- The showcase no longer adds its project repo as a scan root; its PRs arrive through the session working-directory pass, which the showcase coverage test checks. Four project nodes' provenance moved from `git` to `agent_deck` in the fixture as a result.
- Verified live with a logging `gh` shim: twelve calls for the six repos this machine's sessions touch.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Forge discovery runs over every repo the graph found, after the working-directory pass, so repos reached only through session or mux working directories get their PRs; calls are bounded at four at a time and cached by repo set. Verified with `just check` (a graph-checkout test, the cache test, the showcase without its scan-root workaround) and a live run that queried all six of this machine's repos.
<!-- SECTION:FINAL_SUMMARY:END -->
