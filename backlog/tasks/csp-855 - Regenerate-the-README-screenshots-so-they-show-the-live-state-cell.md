---
id: CSP-855
title: Regenerate the README screenshots so they show the live-state cell
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 21:29'
updated_date: '2026-11-14 02:51'
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
- [x] #1 `just screenshots` has been run and every image under docs/images/ shows the current TUI, live marks included
- [x] #2 The README caption under the session screenshot names the live-state mark beside the link glyphs
- [x] #3 tests/showcase_screenshots.rs passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Run `just screenshots` (termshot 0.6.1 from the dev shell) at the branch's final state; it renders the showcase fixture world in a scratch environment (HOME=/fixture, scratch XDG dirs, tmux and forge off), never the operator's data.
2. View each PNG under docs/images/: current TUI, live marks in the live cell, the header's `! 4`, nothing cut or broken, no ADR or backlog IDs or Conspectus docs paths.
3. README caption under the session screenshot: name the live-state marks beside `⊸` and `∘`; check the alt texts still describe the images.
4. tests/showcase_screenshots.rs, git diff --check, checks.sh, just-check.sh; commit images and caption.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Since CSP-829 (ADR 0213) the session screenshot's right pane also lists the live fields (`state`, `since`, `source`, `evidence`, `subagents`) and shows fewer transcript lines, and the `related-preview` key script walks ten fields (`jjjjjljjjj<Tab>jjjjjjjjjj`); the recipe and tests/showcase_screenshots.rs already use it.

Ran `scripts/dev-env.sh just screenshots` (termshot 0.6.1 from the dev shell; the recipe renders the showcase fixture with HOME=/fixture, scratch XDG dirs, tmux and forge off) at the branch's final state, after CSP-880 and CSP-859. All eight PNGs changed; a second run was byte-identical. Viewed each: the live cell (`!`, `»`, `›`, `·`, `×`) on session and mux rows, `! 4` in the header and `! N` on group headers, the live fields (state, since, source, evidence, subagents) in the session's Node zone, the related-preview cursor on `associated with` over the checkout's git status. No ADR or backlog IDs or Conspectus docs paths (the `docs/` in related-preview is the showcase checkout's listing). The termshot font draws `⌥` as a box in prs-view.png, as the old image did; filed CSP-881. `▲` draws fine, so the caption's note that it showed as a box is gone. The Search alt text no longer promises per-row snippets (they give way at 150 columns). tests/showcase_screenshots.rs passed (6 tests); checks.sh clean; just check passed (5706 passed).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Regenerated all eight README screenshots under docs/images/ with `just screenshots` from the showcase fixture world. They now match what `conspectus tui` draws: the live-state mark on session and mux rows, the header's `! 4`, the session's live fields, and the framework-era layout. The README caption under the session screenshot names the live-state marks (`!` waiting, `»` working, `›` ready) beside `⊸` and `∘`, and drops the stale note that the font drew `▲` as a box. The Search image's alt text now matches the image.

Verified by viewing each PNG, a byte-identical second run, tests/showcase_screenshots.rs, checks.sh, and just check (5706 passed). Out of scope: the termshot font draws `⌥` as a box in prs-view.png, as before (CSP-881).
<!-- SECTION:FINAL_SUMMARY:END -->
