---
id: CSP-801
title: 'Cut a snippet inside a word only beside a token, not beside ordinary prose'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-31 02:47'
updated_date: '2026-10-31 03:33'
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
- [x] #1 For text that CSP-800 did not need to change (ordinary spaced prose, including a short word that is too wide for its room), the fit is byte-identical to its behavior before CSP-800, checked by a test over a fixed generated English corpus
- [x] #2 Unspaced CJK text, a long path, and a long URL beside the match still keep context on that side, as CSP-800's tests require
- [x] #3 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Corpus golden first: a deterministic generated English corpus (fixed-seed xorshift over a word list with ordinary punctuation, hyphenated words, contractions, abbreviations, decimals, times, quotes and brackets; whole-word, in-word, and two-word matches) fitted at every budget from 0 to past the text's width. Record the fits with the pre-CSP-800 fit (old snippet.rs swapped in once, then restored) into a golden under crates/tuikit/src/text/goldens/, checked with testkit::golden::assert_golden_text (TUIKIT_ACCEPT_GOLDENS=1 to accept). Confirm the CSP-800 fit fails it.
2. Narrow the last-resort cut in tuikit::text::snippet::crop: when the only edge on a side is the one beside the match and blanks separate the cut run from the match, the cut stays between graphemes only if that run (the whole stretch between blanks) is token-like: it holds a wide character, is wider than a word ever is (a documented cell threshold), or joins names with a path or address separator (/, \, _, ., :) followed by letters. Every other run gives the side up as before CSP-800.
3. Tests: the golden (AC1); CSP-800's CJK, path, and URL tests unchanged (AC2); restore the English at(16) case and the CLI word-edge test to their pre-CSP-800 strings; unit tests for the token rule (separators, abbreviations, decimals, times, the threshold, wide characters); adapt the context property test so it guarantees context for token-like runs and for runs that fit whole.
4. Docs: fit_snippet rustdoc, crates/tuikit/docs/controls/text.md, docs/tui-columns.md column prose plus `just tui-columns` frames, operations guide, CHANGELOG entry, ADR 0155 status, operator-review row.
5. Before/after frames of the content-search modal and Search view on the showcase fixture and a CJK fixture at 100x30 and 60x20 into /tmp/tuifw-baseline/frames-801-*; full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Corpus golden first: `crates/tuikit/src/text/snippet_prose_tests.rs` generates 40 English sentences (fixed-seed xorshift; short, common, and long words, hyphenated words and contractions, figures and abbreviations, closing punctuation, quotes and brackets, em-dash joins, a double space after a full stop now and then), each with a whole-word, an in-word, and a two-word match: 120 cases, 13,956 fits at every budget from 0 to past the text's width. The golden `crates/tuikit/src/text/goldens/prose_snippets.golden` (203 KB, one line per run of budgets with the same fit) was recorded by swapping the pre-CSP-800 `snippet.rs` (`git show 6a8b69cd^:...`) in once and running the test with `TUIKIT_ACCEPT_GOLDENS=1`, then restoring the file; no scratch worktree was needed. CSP-800's fit differs from it in 1,283 fits; the narrowed fit in none. A one-off run over 400 sentences (133,890 fits, not committed) also matched the pre-CSP-800 fit exactly.
Narrowing in `crop` (`crates/tuikit/src/text/snippet.rs`): when the only edge on a side is the one beside the match and blanks separate the cut stretch from the match, the cut stays between graphemes only if `is_token` holds for the whole stretch between blanks (a wide character; wider than `WORD_CELLS` = 24 cells; or a `/`, `\`, `_`, `.`, `:` with a character before it and two letters after it). Otherwise the side goes, exactly the pre-CSP-800 move. Threshold justified from the search fixtures: the widest prose stretch in the showcase and harness fixture messages is 17 cells (`hook-supersession`); their paths and URLs run 10 to 60 cells and all carry separators.
Deliberately updated: snippet_tests `a_cut_still_moves_to_a_word_edge_that_keeps_context` at 16 back to `…quokka…`; `cli::search` `a_cropped_snippet_keeps_the_match_and_cuts_on_word_edges` back to its pre-CSP-800 loop plus `…needle…` at 14; the text_tests context property now expects context only for a token, a stretch touching the match, or one that fits whole (fails if the token exception is disabled); tui-columns frames regenerated, back to the pre-CSP-800 strings. New tests: the COUNTING example (`…wombat ten…` up to 21 cells), token vs word at one budget, and the token rule's examples (paths, URLs, snake_case, std::fmt, a hash, a dashed id, CJK, emoji, and/or vs e.g., U.S., 3.14, 10:30, 24/7, closing punctuation, a 24/25-cell run).
Before/after frames: /tmp/tuifw-baseline/frames-801-{before,after} (content-search modal sessions and messages lists, Search view sessions and messages groupings, at 100x30 and 60x20, on the showcase fixture with query `the` and on claude state with CJK, long-token, mixed, and English messages with query `wombat`). Only English rows changed; CJK, path/URL, and mixed rows are byte-identical.

Checks: full isolated suite 4824 passed, 0 failed, 1 ignored (no rerun); `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo clippy -p tuikit --all-targets -- -D warnings` (features off); `just tuikit-features`; `cargo test -p tuikit --test independence`; `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`; `cargo build --examples`; the whitespace diff check on the tree and on each staged commit. Commits 4978ad8a (fix, tests, golden, text page, tui-columns), bee0f76c (changelog, operations guide, ADR 0155 status, operator-review).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A cut snippet splits a word only beside a token. CSP-800's last-resort cut between graphemes (taken when the only edge on a side is the one beside the match) now applies only when the stretch between blanks there is a token: it holds a wide character, it is wider than 24 cells, or it has a `/`, `\`, `_`, `.`, or `:` with a character before it and two letters after it (`tuikit::text::snippet::is_token`). Any other stretch goes whole and its side shows nothing, the pre-CSP-800 move, so spaced prose is cut exactly as it was; CJK text, paths, URLs, code names, and hashes keep the part that fits.

- AC #1: a golden over a fixed generated English corpus (`crates/tuikit/src/text/goldens/prose_snippets.golden`; 120 cases, 13,956 fits at every budget from 0 to past the width; short, common, and long words, hyphenated words, contractions, abbreviations, figures, punctuation, quotes) recorded by running the pre-CSP-800 fit once. CSP-800's fit differs from it in 1,283 fits, the new fit in none; a one-off 400-sentence run (133,890 fits) matched too. A test checks every stretch in the corpus is a word by the rule.
- AC #2: CSP-800's CJK, path, and URL tests pass unchanged; the CLI wide-character test too.
- Tests added: the token rule's examples (and/or is a token; e.g., U.S., 3.14, 10:30, 24/7, closing punctuation are not; 24 vs 25 cells), a token and a word of one width at one budget, the spelled-out-numbers case. The context property test now expects context for a token, a stretch touching the match, or one that fits whole. Deliberately restored: the English at-16 case (`…quokka…`), the CLI word-edge test (`…needle…` at 14), the tui-columns frames.
- Docs: rustdoc, framework text page (token rule, Thai seam), tui-columns prose, operations guide, CHANGELOG, ADR 0155 status, operator-review CSP-801 row (judgment calls: the 24-cell threshold, the two-letters separator rule, and/or as a token; how to switch each).

**Regressions:**
- None seen in the frames. By the rule, a token with no separator and at most 24 cells (a short kebab-case or CamelCase name, a Thai phrase) that is too wide for its room shows nothing again, as before CSP-800.

**Consistency wins:**
- English is cut only at whole words again, on every surface. Content-search modal at 60x20: `…the notes…`, `…the last…`, `…the fork…`, `…the empty…` (were `…pand the notes…`, `…ince the last…`, `…over the fork…`, `…ing, the empty…`); at 100x30 `…the empty index,…` (was `…rsing, the empty index,…`). Search view at 60x20: `…the notes into a…`, `…the empty index,…` (were `…xpand the notes into a…`, `…sing, the empty index,…`); at 100x30 `…the notes…`, `…they pass.` (were `…and the notes…`, `…ce they pass.`). English rows beside CJK ones: `…wombat ten…` (was `…ine wombat ten…`), `…wombat approach for…` (was `…reful wombat approach for…`). tui-columns frames: `…the notes into a…`, `…feature notes,…`. CLI table: `…needle…` at 14 cells. A stored English snippet (`snippet.text` in the JSON) follows the same fit; the JSON shape is unchanged.
- CJK, path, URL, and mixed rows are byte-identical in every frame.

Checked with `conspectus tui --snapshot`: /tmp/tuifw-baseline/frames-801-{before,after} (modal sessions and messages lists, Search view sessions and messages groupings, 100x30 and 60x20, showcase fixture and a CJK/long-token/English fixture). Full isolated suite 4824 passed, 0 failed, 1 ignored; fmt, both clippy runs, `just tuikit-features`, independence guard, doc, examples, and the whitespace diff check pass.
<!-- SECTION:FINAL_SUMMARY:END -->
