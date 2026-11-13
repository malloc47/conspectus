---
id: CSP-831
title: 'Live state from the tmux pane title (Codex, Claude Code)'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 05:01'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - docs/adr/0207-live-state-sources-beyond-hooks-need-their-own-decision.md
  - docs/plans/live-state/research/02-codex.md
priority: low
type: enhancement
ordinal: 917000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Codex writes a spinner into the terminal title while working and `[ ! ] Action Required` when it needs the operator. It is the best hook-free waiting signal and the only one that clears the instant an approval is answered. Reading it is an already-visible status surface that ADR 0028 allows only through its own decision.

Scope: An ADR (tier under ADR 0086, states the source may assert, how it yields to hook evidence), the tmux format variable read in the existing mux probe, a classifier with a golden string table, the `unknown` fallback for unrecognized titles, and the hook-health diagnostic.

TUI changes: Waiting is detected without hooks for Codex, and clears promptly after an approval.

Tests and checks: String table tests; priority rules against hook evidence.

Blockers: Operator decision 11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An ADR (next number) records the Codex pane-title source: its ADR 0086 tier and in-memory handling, the states it may assert, how it yields to hook evidence (with the two-probe demotion), its cadence, and the health line; it is indexed in docs/adr/README.md and ADR 0207's status points at it
- [x] #2 The tmux mux probe reads the active pane's id and title in its existing list-sessions call: no new process, tmux call, subcommand, or timer; docs/capabilities.toml needs no new program and the capability tests pass
- [x] #3 A pure classifier maps a Codex pane title to waiting (a [ ! ] or [ . ] Action Required prefix, optionally after a mic dot), working (a braille spinner frame in the activity position), or no evidence (anything else); a golden table covers the research strings, empty, the screen fallback, a custom title, and titles that merely contain a spinner glyph or the words Action Required later
- [x] #4 Only panes whose program is codex are classified, the title text never leaves the parser (a sentinel test finds it in no node, probe, binding, diagnostic, JSON, rkyv bytes, or Debug output), and an environment switch turns the read off
- [x] #5 A Codex pane with no hook entry binds as a pane-only waiting (detail input) or working entry with source pane_title, observed at the probe time and with no pid, so it ages by the 15-minute rule; the existing cell, header count, W jump, node show, and graph JSON show it unchanged
- [x] #6 With hook evidence for the same pane: a fresh hook entry wins; a quiet or stale one gives way to a title that says waiting or working; a hook-derived waiting turns working once two consecutive probes after it show the same mark without Action Required; hook ready, error, compacting, starting, and exited never change; tests cover each rule
- [x] #7 A real private tmux server test sets a pane title and the CLI classifies it, skipping when tmux is not installed and never touching the host server
- [x] #8 hook status codex says when pane titles report agents; docs/operations.md, docs/design.md, CHANGELOG.md, and the live-state plan describe the source and its limits; tui --snapshot frames of the default views are unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0217 first: Codex pane title as a Tier 2 source reduced in memory to a closed mark; asserts waiting/input and working only; combination rules (fresh hook wins; quiet/stale hook yields; two-probe demotion of a hook wait; terminal hook states untouched); cadence of the mux probe; health line; opt-out switch. Index it, amend ADR 0207 and 0086 status lines, cite it on the tmux capability entry.
2. Model: LiveSource::PaneTitle; neutral TitleMark and PaneTitle (pane id, mark, observed, run start, run length) in model::live; MuxProbe.titles keyed by session name; FORMAT_VERSION 13.
3. Classifier: HarnessAdapter::pane_title_mark (default none); Codex override in a codex_title module with a golden table.
4. Provider: append #{pane_id} and #{pane_title} to the list-sessions format; the parser classifies through the pane program's adapter and drops the text; TmuxDiscovery records the marks on its probe in ms and carries each pane's run across probes through DiscoveryCaches; CONSPECTUS_DISABLE_PANE_TITLES drops the title from the format.
5. Binding: bind_live folds probe titles into node_live/pane_live after the store's winners, matching hook entries by tmux server, session, and pane.
6. Surfaces: glossary for the new source, hook status codex message, docs (operations, design, CHANGELOG, plan README).
7. Tests: classifier table, parser and carry, binding rules, sentinel privacy, real private tmux CLI test; run checks.sh and run-tests.sh; frames unchanged.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 11: approved. Write the ADR first (tier under ADR 0086, the states the source may assert, how it yields to hook evidence), then the tmux format-variable read in the existing mux probe with a golden string table.

Landed ADR 0217 (688df29d), the classifier and vocabulary (f919794d), the probe read and binding (72b14b66, format 13), the hook status note (4297ad82), and the private tmux CLI test (9799d0a8). Each commit passes `cargo check --workspace --all-targets --all-features` (checked with `git rebase -x`). Design change while building: the two-probe demotion counts the last two probes of the run after the hook entry's evidence (`previous_ms`), not the run's start, so an approval answered before any probe saw the marker still clears; the ADR was corrected in its own commit.

Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) exit 0; run-tests.sh (cargo test --workspace --all-targets --all-features) exit 0, 5566 passed, 0 failed, 1 ignored, the traced capability test included; `just tuikit-features` exit 0. Three timing tests failed once in a loaded narrow run (a forge gh concurrency count, two TUI consistency tests whose header read `scanned 1s ago` against `0s ago`) and passed alone and in the full run. frames.sh: the nine showcase frames are byte-identical between a build of a0754e93 and this branch. Manual check on a private tmux server with a stand-in tmux reports as `codex` and the title `[ ! ] Action Required ...`, no hooks: the Mux view shows `!` on the mux row and `! 1` in the header with `W next waiting`, the detail pane lists `pane_agents %0 waiting · input`, `node show` gives `source: pane_title`, `table mux --columns +live` reads `1 waiting`, and `hook status codex` ends `; 1 agent shown from its pane title`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Codex's pane title is now a live-state source (ADR 0217, the first under ADR 0207 decision 4). The mux probe's existing `tmux list-sessions` format gains `#{pane_id}` and `#{pane_title}`; for a pane whose program is `codex`/`codex-rs`, the parser hands the title to the Codex adapter (`pane_title_mark`), which reads only the leading activity segment (`[ ! ]`/`[ . ] Action Required` after an optional `● ` is waiting, a braille spinner frame is working, anything else neither) and the text is dropped at once. The probe records the mark and its run (`since_ms`, `previous_ms`, carried in the discovery caches) on `MuxProbe::titles`; `bind_live` folds the marks in after the store: pane-only `waiting/input` or `working` with source `pane_title` and no pid when the pane has no hook entry, a fresh hook entry wins, a quiet/stale/unknown one gives way, and a hook-derived wait turns working once two consecutive probes after it lose the marker; hook ready, error, compacting, starting, and exited never change. `hook status codex` adds how many agents pane titles show; `CONSPECTUS_DISABLE_PANE_TITLES` turns the read off; `graph.bin` is at format 13. No new program, tmux call, or timer.

Verified by the classifier's golden table, provider, carry, binding, discovery, and sentinel privacy tests, a CLI test against a private tmux server, checks.sh, the full suite (5566 passed), `just tuikit-features`, byte-identical showcase frames, and a manual TUI snapshot on a private server. The title strings come from Codex's source and are unverified against a running Codex (CSP-822 now asks for them); Claude Code's titles are not read (CSP-869). Follow-ups: CSP-868 (raise a fresh hook working to waiting on the title, needs a decision), CSP-869, CSP-870 (non-active panes, wrapped launches).
<!-- SECTION:FINAL_SUMMARY:END -->
