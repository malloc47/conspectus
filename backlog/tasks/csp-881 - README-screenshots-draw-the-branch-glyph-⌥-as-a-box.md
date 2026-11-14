---
id: CSP-881
title: README screenshots draw the branch glyph ⌥ as a box
status: To Do
assignee: []
created_date: '2026-11-14 02:48'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 958000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`just screenshots` (termshot 0.6.1, which draws with its bundled font and has no font option) draws `⌥` (U+2325, the branch glyph: a PR row whose branch exists locally, and the Related row `has PR ⌥ refs/heads/...`) as an empty box in docs/images/prs-view.png. The image rendered before CSP-855 shows the same box, so this is not new. The other glyphs in the eight screenshots draw correctly, `▲` included (the README caption used to say the font drew `▲` as a box; CSP-855 dropped that note because it no longer does).

Options, for the operator: accept it and say so in the caption under the image table; render the PNGs with a tool or font that has U+2325 (a new dev-shell tool needs an ADR under ADR 0113's pipeline); or pick a branch glyph the bundled font has (a TUI glyph change, docs/tui-columns.md glyph table and vetted-glyph rules apply). Found while doing CSP-855.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 docs/images/prs-view.png draws the branch glyph, or the README says why it shows a box
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
