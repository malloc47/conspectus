---
id: CSP-334
title: '`TranscriptDocument` + `TranscriptTurn` model + `SessionLocator` types'
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 209000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/viewer/model.rs` fleshed out with full types,
serde derives, Display impl, and round-trip tests. Public
surface: `SessionLocator` (`ClaudeCode`/`Codex`/`OpenCode`
variants, `serde(tag = "harness", rename_all = "kebab-case")`
with `OpenCode` overridden to serialize as `opencode` matching
conspectus's harness key), `TranscriptTurn` (`role`, `kind`,
`body`, `timestamp: Option<DateTime<Utc>>` skip-on-none),
`TurnRole` (`User`/`Assistant`/`System` with `header_label()`
matching the inline-preview labels), `TurnKind`
(`Message`/`CompactionSummary`/`ToolUse`/`ToolResult`/`Thinking`
with `shown_by_default()` filtering noise),
`TranscriptDocument` (`meta` + `turns` plus an
`unavailable(locator)` constructor for parser-failure
fallback), `TranscriptMeta`. Zero conspectus-graph types
appear in the public surface. `chrono = 0.4` added to
`Cargo.toml` with `default-features = false, features =
["serde", "alloc"]` — already pre-listed in
`ALLOWED_EXTERNAL_DEPS` so the dep-surface test still
passes. Eleven model tests cover Display, harness_key,
round-trips for all three locator variants, kind/role
helpers, timestamped + omitted-timestamp turn serde,
`unavailable()` shape, and a full document round-trip.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-002`
