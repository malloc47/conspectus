---
id: CSP-881
title: README screenshots draw the branch glyph ⌥ as a box
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14 02:48'
updated_date: '2026-11-15 01:15'
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
- [x] #1 docs/images/prs-view.png draws the branch glyph, or the README says why it shows a box
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator, 2026-11-14: note it in the caption (option 1). The PRs screenshot stays as rendered; a line under the README image table says why the branch glyph is a box.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a line under the README's screenshot table saying that the font the screenshots are rendered with (termshot's bundled font) has no branch glyph `⌥`, so the PRs view shows an empty box where a terminal draws it, as the operator chose on 2026-11-14. No image, tool, or TUI glyph changed. Verified with `git diff --check`; the text names no ADR, backlog ID, or docs path.
<!-- SECTION:FINAL_SUMMARY:END -->
