---
id: CSP-769
title: >-
  Decide how quick select and Reveal treat fork entities now that no view lists
  them
status: To Do
assignee: []
created_date: '2026-10-28 05:01'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
priority: low
type: chore
ordinal: 855000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Retiring the Forks view (CSP-711) left `actions::natural_view` with no home for a live fork. Fork nodes are still in the quick-select corpus (`search_entities.rs`) and can be pinned (`PinKind::Fork`), so a fork that quick select cannot reach along a relationship path now reports "in Sessions view" and then fails on Enter with "isn't listed in the Sessions view". The Pinned row menu also still offers "Reveal in its view" for a fork pin, which fails the same way. Options: leave unreachable forks out of the corpus, give the open plan a "not listed" outcome, hide the Reveal entry for kinds no view lists, or give forks a home (for example as rows in the Sessions graph). The operator picks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Quick select never promises a view that does not list the entity it found
- [ ] #2 The Pinned row menu offers Reveal only when the entity is listed in some view
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
