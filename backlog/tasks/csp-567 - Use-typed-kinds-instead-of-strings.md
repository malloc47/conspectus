---
id: CSP-567
title: Use typed kinds instead of strings
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 576000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: the node explorer carries `neighbor_kind` as
  `&'static str` or `String` (`"mux_session"`) and parses it back
  with `NodeKind::from_snake_case` at render time, although
  `NodeKind` exists. Mux-attribution evidence is an
  `Option<String>` ranked by matching against the
  `resolve::evidence` string constants, so a typo misranks silently
  instead of failing to compile.
- Plan: thread `NodeKind` through the explorer view models; add an
  `Evidence` enum with `#[serde(rename_all = "snake_case")]` so the
  graph JSON and `graph.bin` wire format stay the same.
- ADR: evidence is part of the serialized model. Confirm the format
  is unchanged with the graph snapshot tests, and bump the
  `graph.bin` format version if the rkyv layout changes (ADR 0083).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`model::MatchKind` names the 16 mux-attribution match
kinds that adapters actually emit, and `SourceMetadata::match_kind()`
reads one back. Producers in `cross_link`, `codex_log`, and
`hook_sidecar` and the resolver's rankers and filters use it, with
exhaustive `match`es; `resolve::evidence` is gone. On the wire it is
still the same snake_case string, so the graph JSON and
`graph.bin` are unchanged (no ADR 0083 bump). `SourceMetadata.evidence`
stays free-form text, because most adapters write prose there
("gh pr list head ref"). `NodeKind` moved from `tui::icons` into
`model`. The detail and explorer view models carry it instead of
`&'static str` labels, and the four duplicated `kind_label`
helpers are gone. The explorer's relationship-group key stays a
string, because unresolved endpoints can name non-node types
(`path`). Typing the match kinds surfaced `CSP-573`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-014`
