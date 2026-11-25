---
id: CSP-908
title: 'TUI: a live mux can preview the transcript of the agent session it hosts'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-25 18:30'
updated_date: '2026-11-25 18:51'
labels:
  - tui
  - config
dependencies: []
priority: medium
type: enhancement
ordinal: 983000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A running mux with an agent in it previews its pane capture, which is the agent's own TUI drawn inside the Preview: its borders, status line, and colors sit beside Conspectus's own and can read as clutter or as a second interface competing with the first. The operator (2026-11-25) wants to opt in, through configuration, to the Preview a mux already gets when it is not live: the session view, which is the linked session's transcript through the native viewer. The pane would otherwise be their only way to see what the agent is doing from the Mux view.

Related: ADR 0112 (what each node previews, and `[tui] session_preview` for a session in tmux, the opposite direction), ADR 0253 (a stopped mux already previews the session it hosted last), `v` on a mux row (opens the preferred linked session, which this Preview should match). Neither the pane nor a transcript refreshes on a timer today; both load when the cursor lands, on `r`, and when a tmux hand-off returns (`RunConfig::mux_preview_interval` is parsed and unused, backlog CSP-185), so "the same cadence" is the same triggers, and the transcript source must take part in them.

Out of scope: a controls-overlay section or remembered choice for it (config only, as asked), a timer-driven refresh (CSP-185), and the mux that hosts this TUI, which previews nothing today.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `[tui] mux_preview = "pane" | "transcript"` is a config key, `pane` by default, and an unknown value is a config diagnostic that keeps the default
- [x] #2 With `transcript`, a live mux that hosts an agent session previews that session's transcript wherever a live mux previews its pane: the Mux view's rows, the Pinned view's row for a pin on the mux, a drilled-in mux node, and a Related row's neighbor (the History view lists only what is gone, and no view builds a mux header row, so those have no live mux to preview)
- [x] #3 The session shown is the one `v` opens for that mux; a live mux with no agent session, or whose agent's harness the viewer cannot read, still previews its pane
- [x] #4 The transcript reloads when the mux's pane would have: when the cursor lands on the row, on `r`, and when a tmux hand-off returns; it scrolls with `J`/`K` and is hidden by `--no-live-preview` as any transcript is
- [x] #5 Under `pane` nothing changes, and under `transcript` a session row (`session_preview` decides it), a stopped mux (ADR 0253), and the mux that hosts this TUI preview as before
- [x] #6 Tests cover the source in each place above, the sparse cases, the config key, and a rendered frame; the operator guide, design.md, an ADR, and the changelog say what the key does
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Config: `MuxPreview { Pane (default), Transcript }` beside `SessionPreview` in `src/tui/mod.rs` (`as_str`, `parse`); `[tui] mux_preview` in `TuiFile`/`TuiConfig`/`merge_tui` with a diagnostic for an unknown value; `RunConfig::mux_preview`, set in `cli/tui.rs`, `dev_scenarios.rs`, and `RunConfig::defaults`. No App field, controls section, or state-file entry: it is config only.
2. Resolver (`src/tui/preview/mod.rs`): one helper, `live_mux_source(app, mux)`, turns the `Pane(mux)` a live mux resolves to into `Transcript(session)` when the setting is `transcript`, the mux has a session `v` would open (`actions::viewer_session_for`, made `pub(crate)`), that session is a live node, and its harness has a transcript parser (a new `viewer_bridge::can_read` predicate); otherwise it stays `Pane(mux)`. Apply it at the two places that produce a mux pane source: `node_source`'s `MuxSession` arm and `selection_source`'s mux-row arm. A mux hosting this TUI, a stopped mux, and session rows resolve as before because the swap acts only on a pane source those paths already produced.
3. Cadence: no code expected, since `plan_preview_load` treats a transcript like any source and `Msg::HandoffReturned` already marks the linked sessions' transcripts stale. Prove it with tests (load on landing, `r`, hand-off return) rather than assume it.
4. Tests (red-check against the old behavior): config default/valid/invalid; source per place (Mux view row, mux header in the Sessions view, History view, drilled-in node, Related neighbor, pin bound to a mux); several sessions pick `v`'s; no session, a gone session, and an unreadable harness keep the pane; default unchanged; hosts-this-TUI, stopped mux, and session rows unchanged; `--no-live-preview`; J/K anchor; cadence; a rendered frame.
5. Docs: ADR 0256 (amends ADR 0112's mux row) with the README index, `docs/operations.md` key table and prose, `docs/design.md` preview paragraph, CHANGELOG; the doc comments on `node_source` and `selection_source`.
6. Verify frames with `tui --snapshot` over a showcase world, then `just check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Where the work landed: `MuxPreview` and `RunConfig::mux_preview` (`src/tui/mod.rs`), the `[tui] mux_preview` key and its diagnostic (`src/config.rs`), `viewer_bridge::can_read`, `actions::preferred_session_for_mux` made `pub(crate)`, and `live_mux_source` / `hosted_transcript` in `src/tui/preview/mod.rs`, called from `node_source`'s mux arm and `selection_source`'s mux-row arm. Acting only on a pane source those paths already produced keeps a stopped mux, a session row, and the mux hosting this TUI exactly as they were.
Cadence: no code. Neither a pane nor a transcript refreshes on a timer today (`RunConfig::mux_preview_interval` is parsed and unused, CSP-185); both load on landing, `r`, and a hand-off return, and `Msg::HandoffReturned` already marks the linked sessions' transcripts stale. `mux_transcript_cadence` holds the two to the same sequence of loads. ADR 0250's remark of a capture "every two seconds" does not match the code.
Corrections to my own task text while building: the History view lists only gone records and no view builds a mux header row, so neither is a place a live mux previews; criterion 2 now says so.
Finding, not changed: `Confidence` derives `Ord` with `High` first, and every preferred-link comparator sorts `right.confidence.cmp(&left.confidence)`, so at equal provenance a Low link outranks a High one (`pick_preferred`, `GraphSnapshot::preferred_mux_for`, `preferred_session_for_mux`, the resolver's comparators). No test pins it. It surfaced because my first multi-session test used High confidence to make a link win and lost. Tests here rank by provenance instead.
Validation: `just check` green after the last change (6037 passed, 1 skipped; fmt, clippy, rustdoc -D warnings, diff check; tuikit-features skipped, nothing under crates/tuikit changed). 17 new unit tests plus `tests/tui_session_preview.rs` (config file to rendered Preview). With the swap disabled 9 of the new tests fail, and the sparse-case guards fail when their liveness and readability conditions are dropped. Real frames via `tui --snapshot` over the showcase fixture: default holds the pane's place, the key shows the hosted session's transcript, a bad value warns and names the choices. Not run locally: CI's second pass, `cargo test --workspace`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`[tui] mux_preview = "transcript"` makes a live mux that hosts an agent preview that agent's transcript, in Conspectus's styling, in place of the pane's capture. The session is the one `v` opens on the mux, and it applies in the Mux view, on a pin's row in the Pinned view, on a drilled-in mux, and on a Related row. The default is `pane`, as before. A live mux with no live agent, or whose harness the viewer cannot read, keeps its pane; a stopped mux, a session row, and the mux hosting this TUI preview as they did. The transcript loads when the pane would have (cursor landing, `r`, a hand-off return), since neither source has a timer. Config only, with no overlay control. ADR 0256 records it, amending ADR 0112; the operator guide, design.md, ADR index, and changelog are updated.
Verified with `just check` (6037 passed, 1 skipped; fmt, clippy, rustdoc, diff check), 17 new unit tests plus an end-to-end test from a config file to the rendered Preview (9 fail with the swap disabled), and real `tui --snapshot` frames. Not run locally: CI's `cargo test --workspace`.
<!-- SECTION:FINAL_SUMMARY:END -->
