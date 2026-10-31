---
id: CSP-801
title: 'Cut a snippet inside a word only beside a token, not beside ordinary prose'
status: To Do
assignee: []
created_date: '2026-10-31 02:47'
labels:
  - tui-framework
  - tuikit-text
milestone: m-25
dependencies:
  - CSP-800
priority: medium
type: bug
ordinal: 887000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-800 made the shared snippet fit keep context on a side whose only word edge is next to the match, by cutting between characters. That is what unspaced CJK text and long tokens (paths, URLs) need, but it also changed ordinary spaced prose: where a short English word is the only context on a side and is too wide for its room, the snippet now shows a cut word (a snippet that read '...wombat ten...' now reads '...ine wombat ten...', and about 15% of fits on generated English text changed). A half word is a worse result than an empty side for prose, which is the most common case, and the owner asked that spaced text stay identical. Narrow the rule so the character-level cut applies only to a run that is token-like (it contains a wide character, or it is longer than an ordinary word, or it is made of path/URL separators) and leave every other fit exactly as it was before CSP-800.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 For text that CSP-800 did not need to change (ordinary spaced prose, including a short word that is too wide for its room), the fit is byte-identical to its behavior before CSP-800, checked by a test over a fixed generated English corpus
- [ ] #2 Unspaced CJK text, a long path, and a long URL beside the match still keep context on that side, as CSP-800's tests require
- [ ] #3 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
