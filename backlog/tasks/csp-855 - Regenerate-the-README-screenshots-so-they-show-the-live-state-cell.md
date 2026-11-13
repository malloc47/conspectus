---
id: CSP-855
title: Regenerate the README screenshots so they show the live-state cell
status: To Do
assignee: []
created_date: '2026-11-12 21:29'
updated_date: '2026-11-13 00:24'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
ordinal: 940000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-823 gave the showcase world live state for every mark and drew a live-state cell after the age on session and mux rows (ADR 0209), so `docs/tui-columns.md` and the fixture frames show the marks, but the README's PNG screenshots under `docs/images/` were last rendered before it (and before several framework-era layout changes) and no longer match what `conspectus tui` draws. The README caption under the first screenshot explains `⊸` and `∘` and could name the new mark too.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `just screenshots` has been run and every image under docs/images/ shows the current TUI, live marks included
- [ ] #2 The README caption under the session screenshot names the live-state mark beside the link glyphs
- [ ] #3 tests/showcase_screenshots.rs passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Since CSP-829 (ADR 0213) the session screenshot's right pane also lists the live fields (`state`, `since`, `source`, `evidence`, `subagents`) and shows fewer transcript lines, and the `related-preview` key script walks ten fields (`jjjjjljjjj<Tab>jjjjjjjjjj`); the recipe and tests/showcase_screenshots.rs already use it.
<!-- SECTION:NOTES:END -->
