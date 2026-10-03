---
id: CSP-566
title: Strip backlog IDs from code comments
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 575000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: 712 comments still open with a backlog or wave ID
  (`// CSP-467 wave 7: consult SnapshotIndex ...`). The rationale
  is useful; the history belongs in commits and this backlog. The
  largest concentrations are `tui/app.rs` (73), `discovery/mod.rs`
  (55), `tui/keymap.rs` (29), `model/mod.rs` (28),
  `discovery/harness/mod.rs` (28), `tui/runtime.rs` (27), and
  `tui/ui.rs` (26).
- Plan: one commit per module; keep the "why", drop the ID and
  wave; check that the diff touches comment lines only. Consider an
  `AGENTS.md` line ("comments explain why; IDs go in commit
  messages") so new IDs stop accumulating.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0100. About 1,000 comment lines changed. A script
stripped leading `ID:` prefixes and ID-only parentheticals, keeping
any ADR references. The ~330 lines of history narration ("pre-H-EXT-004
if-chain", "wave 2 will…") were rewritten by hand to describe current
behavior, which corrected several stale claims. Five comments keep a
"backlog `ID`" pointer to open work (`CSP-108`, `CSP-171.01/CSP-171.02`,
`CSP-185`, `CSP-318`). `tests/comment_hygiene.rs` fails on new ID
citations, and `AGENTS.md` states the rule. Side finding:
`RunConfig::mux_preview_interval` is parsed but unused until
`CSP-185` lands; the preview module doc now says so.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-013`
