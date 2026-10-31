---
id: CSP-800
title: >-
  Keep the context beside a match when a snippet's text has no space near the
  cut
status: To Do
assignee: []
created_date: '2026-10-31 01:48'
labels:
  - tui-framework
milestone: m-25
dependencies: []
references:
  - crates/tuikit/src/text/snippet.rs
  - src/cli/search.rs
  - src/viewer/search.rs
priority: medium
type: bug
ordinal: 886000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while moving `conspectus search`'s table onto `tuikit::text::fit_snippet` (CSP-774). The fit moves a cut that would split a word to the first blank between the cut and the match, even when that blank is the one next to the match, so all the context on that side goes. Its own doc says the move happens "unless that would give up the room next to the match". Text without spaces near the match loses everything: `日本語のテキストと説明 wombat もっと日本語のテキストと説明がここに続きます` fits a 63-cell budget as `…wombat…`, where the CLI's old centered crop showed `日本語のテキストと説明 wombat…`. A long token beside the match (a path, a URL) does the same in English text.

Every snippet goes through this fit: the TUI's content-search rows and Search view, quick select's match line, the stored hit snippet (`snippet_for`, so `conspectus search --format json`'s `snippet.text` for long bodies), and now the CLI table.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A cut whose only word edge on its side is next to the match keeps context there (cut between graphemes, or at an ideograph boundary) instead of dropping the side
- [ ] #2 Tests in the framework's snippet tests cover CJK text without spaces and a long token beside the match at several budgets; the CLI table's wide-character test asserts context shows
- [ ] #3 Changed frames and stored snippets are reviewed and listed; the JSON shape is unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
