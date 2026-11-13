---
id: CSP-819
title: Keep the operator's key order when hook install edits settings.json
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 15:06'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies:
  - CSP-811
documentation:
  - docs/plans/live-state/research/01-claude-code.md
priority: low
type: enhancement
ordinal: 905000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook init` and `hook remove` rewrite Claude Code's `settings.json` through a JSON builder that sorts object keys, so the operator's file is reordered on the first install. Values survive, but a reordered settings file produces a noisy diff in a dotfiles repository.

Scope: Enable order preservation for the settings edit. `serde_json`'s `preserve_order` feature adds `indexmap` as a dependency, so this needs an ADR first (need, alternatives such as editing the text in place, the dependency's cost), then the change, with the existing install, upgrade, and remove tests asserting byte-stable output for untouched keys.

TUI changes: none.

Tests and checks: Install, upgrade, and remove on a scratch settings file with a deliberately unsorted key order and formatting, comparing untouched sections byte for byte.

Blockers: The Claude Code install.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An ADR records the need and the alternatives
- [x] #2 `hook init` and `hook remove` leave untouched keys in their original order
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Measure option A for real: enable serde_json's preserve_order on a scratch tree state, run the full suite, record what changes, revert.
2. Write ADR 0208 (need, options A preserve_order, B an in-place splice with a scanner of our own, C document only; tradeoffs; decision B; strict JSON only), index it under the write envelope, note in ADR 0206 that its open question is settled.
3. Add src/cli/json_splice.rs: a span scanner for strict JSON, a structural diff of the old and new values that edits only members and elements that changed (cut with their separator, append after the last item in the container's layout, replace changed scalars), parse-back check; unit tests in json_splice_tests.rs, including a seeded randomized round trip.
4. Route the Claude Code install and remove through it (read the text with the value, splice before the dry-run branch so a dry run reports the same errors, pretty-print only a new file).
5. CLI tests in a scratch HOME: unsorted keys, 4-space and tab indentation, CRLF, trailing newline or none, unicode literal and escaped, unrelated keys, operator hooks; bytes outside the hooks value identical, install then remove gives back the original byte for byte, an upgrade edits only the matcher and timeout; project scope.
6. Docs: operations.md, CHANGELOG, plan README section 8; checks.sh and run-tests.sh; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Blast-radius experiment (option A): serde_json preserve_order enabled on a scratch tree state, full suite run: 5372 passed, 10 failed (3 cli_search JSON goldens, 7 html_snapshots payload goldens, all key-order changes); Cargo.lock gained only an indexmap edge for serde_json (indexmap is already a declared, unused dependency). Reverted before any commit. Also noted: Map::remove becomes swap_remove under preserve_order, and the file would still be re-indented.
Chose option B (ADR 0208): src/cli/json_splice.rs splices only the members and elements that differ, refuses non-strict JSON and a key twice in an edited object, and parses the result back. A first CLI test found that one inline object at the top made the whole new hooks value one-line; the rule is now that new items are one-line only when every filled item of the container is.
Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) exit 0; run-tests.sh exit 0, 5398 passed, 0 failed; tui --snapshot frames (frames.sh, 9 views) byte-identical to the last saved set; manual scratch-HOME install/remove of a hand-kept file showed an additions-only diff and cmp-identical file after remove.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`hook init claude-code` and `hook remove claude-code` now edit `settings.json` in place instead of re-serializing it, so the operator's key order, indentation, line endings, escapes, and number spellings survive, and an install followed by a remove gives back the original file byte for byte (ADR 0208).

- ADR 0208 compares (A) serde_json `preserve_order` (measured: no new crate, but 10 goldens change across `search --format json` and the HTML export because every `serde_json::Map` reorders; still re-indents), (B) an in-place splice, and (C) documenting the reordering; it adopts B, keeps strict JSON only, and settles ADR 0206's open question (index row amended).
- `src/cli/json_splice.rs`: a span scanner plus a structural diff that cuts removed items with one separator, appends new ones in the container's layout and the file's indentation unit and line ending, pairs array elements by equality then likeness, and parses the result back before it is written. A dry run builds the same text.
- Tests: 12 unit tests (layouts, cuts, likeness pairing, refusals, 600-seed randomized edits and append-cut round trips) and 4 command-line tests in a scratch home (both scopes; four-space, tab, and CRLF files with and without a trailing newline, unicode, unsorted keys; operator hooks byte for byte; in-place upgrade; byte-identical remove; refusal of comments, trailing commas, and a duplicated key).
- Docs: `docs/operations.md`, `CHANGELOG.md`, plan README section 8.

Verified: checks.sh exit 0; full suite 5398 passed, 0 failed; snapshot frames byte-identical.
<!-- SECTION:FINAL_SUMMARY:END -->
