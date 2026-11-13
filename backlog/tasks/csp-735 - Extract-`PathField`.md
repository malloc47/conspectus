---
id: CSP-735
title: Extract `PathField`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 05:55'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-733
  - CSP-715
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
  - docs/adr/0097-launch-spec-form-primitive.md
priority: medium
type: enhancement
ordinal: 821000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The tab-completed path widget (`path_omnibox.rs`) imports nothing from the app but its key table, yet it is not behaviorally neutral. It suggests files as well as directories and validates with `exists`, so a file passes for a `cwd`. It reads the filesystem and `$HOME` every frame (one `stat` per path row, a `read_dir` while focused) with no injection point. `~/x` validates but is committed unexpanded. `selected`, `MAX_CANDIDATES`, `source`, `exists`, and `expanded_value()` are dead or unrendered. Two renderers differ on the Tab hint (a 32-cell inline hint versus a mark only). Three `~` expansions exist (inventory slice 03, section 2.2).

Scope: Create `PathField` on `LineEdit` with a `PathSource` trait (listing, stat, home directory, so tests need no real filesystem or `$HOME`), a `PathKind` (directory or file), ranked completion with known candidates ahead of filesystem matches, cached results so rendering does no I/O, and `~` expansion on commit through `expanded_value()`. Remove the dead surface or adopt it. Settle one Tab hint rule. Leave the seam for the ranked candidate list that CSP-455 asks for and for the browse mode of CSP-434.

TUI changes:
- Consistency wins: a file no longer validates as a working directory; `~` paths work; no per-frame disk I/O; one Tab hint rule
- Regressions or behavior changes: the inline Tab hint no longer reserves 32 cells in forms that used it, or appears in forms that did not, depending on the rule chosen

Tests and checks: Tests against an in-memory `PathSource` for ranking, completion, kind, tilde, and caching. The path behaviors from the text-entry defect task pass unchanged.

Blockers: `LineEdit` and the text-entry defect task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `PathField` takes a `PathSource` and a `PathKind`, caches results, and does no I/O while rendering
- [x] #2 A file does not validate where a directory is required
- [x] #3 `~` is expanded on commit, and the three `~` implementations are one
- [x] #4 The dead surface (`selected`, `MAX_CANDIDATES`, `source`, `exists`) is removed or used, and `expanded_value()` has a caller
- [x] #5 One Tab hint rule applies to every form that has a path field
- [x] #6 The seam for a ranked candidate list is documented against CSP-455
- [x] #7 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #8 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #9 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Framework: add `tuikit::path_field` with `PathField` on `LineEdit`, the read-only `PathSource` trait (entries, kind, home), `PathKind { Dir, File }`, `Candidate` (host-known path with a priority), `Completion`, `Validity { Unknown, Exists, Advisory }`, and the pure `expand_home`/`collapse_home`. The field looks at its source only when its value changes, on `refresh()`, and when it is built; the directory listing is cached by directory, so typing in the last component re-filters without I/O; rendering reads only the cache.
2. Completion: prefix extensions only (Tab adds the best remainder, and the hint shows exactly that remainder); known candidates by priority ahead of listed entries by name; a `Dir` field lists only directories; `~` text completes in `~` form. `PATH_FIELD_KEYS` (`Tab` complete) joins the standard tables; `candidates()` is the ranked list for the CSP-455 seam.
3. One renderer, `PathFieldView`: the field (TextField), the validity mark (`?` inert, `marks::OK` success, `marks::WARNING` warning), and while focused and a completion exists a `Tab: <remainder>` hint in a reserve of at most 30 cells and half the width, the remainder cut in the middle.
4. Test kit: `testkit::paths::MemoryPaths`, an in-memory tree with call counters; framework tests for ranking, completion, kind, tilde, caching (no source calls while rendering), refresh, and the view.
5. Host: `tui::paths` adapter (std::fs read-only, `$HOME`), `path_omnibox.rs` and `OMNIBOX_KEYS` removed; new mux cwd, mux launch cwd and worktree repo, pin edit path rows hold `PathField` (Dir); `form::path_row`/`path_cells` draw through the view; commit sites call `expanded_value()`; `legacy_pins` and `config` expand `~` through `tuikit::path_field::expand_home`.
6. Docs: control page `crates/tuikit/docs/controls/path_field.md`, architecture row, ADR 0167 + index, ADR 0097/0151 notes, design.md, operations.md/CHANGELOG, operator-review rows.
7. Before/after snapshot frames (new mux, mux launch cwd and repo, pin edit; focused/unfocused; with and without a completion; 100x30 and 60x20) in /tmp/tuifw-baseline/frames-735-*; full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework: `tuikit::path_field` (PathField, PathSource, PathKind, Entry, Candidate, Completion, Validity, PathFieldView, expand_home, collapse_home), `PATH_FIELD_KEYS`/`PathKey` in keys/standard.rs, `testkit::paths::MemoryPaths` (in-memory tree with call counters). 39 unit tests plus doctests; a style-aware golden for the view.
Host: `tui::paths` (SystemPaths over std::fs and $HOME; `directory_field`), path_omnibox.rs and OMNIBOX_KEYS removed, form::path_row/path_cells over the view, new mux / launch spec cwd / launch worktree repo / pin edit path rows migrated, legacy_pins and config expand `~` through the framework's expand_home. Three mux launch insta snapshots updated for the dropped 32-cell suffix.
Frames: /tmp/tuifw-baseline/frames-735-{before,after} (72 each: new mux, launch cwd, launch repo, pin edit; seeded/empty/tilde/tab/dir/file/missing/relative/unfocused; 100x30 and 60x20). 5 identical, 67 changed, by category in the final summary.
Checks: fmt --check, clippy --workspace --all-targets --all-features -D warnings, full isolated cargo test (4035 passed, 0 failed, 1 ignored), nextest (4035 passed, 1 skipped), RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps, git diff --check: all pass.
Follow-up filed: CSP-777 (the launch form's command row still runs the atelier workspace lookup per frame; after CSP-737).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::path_field`: `PathField` on `LineEdit`, asking for a `PathKind` (`Dir`/`File`) and reading the file system only through a read-only `PathSource` (entries, kind, home) shared behind an `Arc`. Completion only extends the typed text: the host's known `Candidate`s by priority, then the source's entries of the typed directory by name; `Tab` (`PATH_FIELD_KEYS`) adds the best remainder, the same text the hint shows; `candidates()` is the ranked list for the CSP-455 seam (browse mode noted on the page). Answers are kept until the text changes or `refresh()` (the listing until the directory changes), so validity, candidates, `expanded_value()`, and drawing do no I/O. `expand_home` is the one `~` rule: the forms commit `expanded_value()`, and the pin store and config loader call it too. `PathFieldView` is the one hint rule. Conspectus supplies `tui::paths::SystemPaths`; every path field is a directory field; `path_omnibox.rs`, `OMNIBOX_KEYS`, `path_mark`, `completion_hint`, and the dead `selected`/`MAX_CANDIDATES`/`source`/`exists` are gone. ADR 0167; page `crates/tuikit/docs/controls/path_field.md`.

Verified with 39 framework tests over `testkit::paths::MemoryPaths` (ranking, completion, kind, tilde, caching: no source call while drawing or reading), host tests over real tempdirs, the CSP-715 `~` tests unchanged, the full isolated suite (4035 passed, 0 failed, 1 ignored), nextest, clippy, rustdoc, and 72 before/after `--snapshot` frames (new mux, launch cwd and worktree repo, pin edit; focused and unfocused, with and without a completion; 100x30 and 60x20) in /tmp/tuifw-baseline/frames-735-*.

**Regressions**
- Typing text that only appears inside a known path (`src` for `/home/x/src/foo`) no longer completes to it; type the path's start.
- While a hint shows, a focused path value gives up to 30 cells (at most half the row) to it, so the value's window narrows and widens as completions come and go; new in the pin edit form, where at 60 columns the value window is 9 cells while hinting.
- At 60 columns the hint is at most half the row, so long remainders show cut (`Tab: /tmp/t…ome735`).
- In the launch form's worktree repo, `Tab` completes before it moves on.
- The advisory mark is `▲` where it was `!`.
- Opening a form looks at each path once (a `stat` and a directory listing) instead of on each frame.

**Consistency wins**
- A file no longer validates as a working directory (`▲`, not `√`), and files are no longer offered.
- `~` paths work end to end: mark, completion, and commit read `~` as home by one rule shared with the pin store and config.
- No per-frame disk I/O from path fields.
- One Tab-hint rule in every form: `Tab: <remainder>` only while focused with a completion, naming exactly what `Tab` adds; the pin edit form shows it too.
- Unfocused path fields, and focused ones with nothing to complete, give the path the 30 cells the old suffix kept: at 60 columns the whole path shows instead of a four-cell window.
- The launch form's worktree repo gains the validity mark and completion.
- The advisory mark is the warning severity's mark; long remainders are cut in the middle (house rule 14).
- At 100 columns a focused path with a short completion looks as before.
<!-- SECTION:FINAL_SUMMARY:END -->
