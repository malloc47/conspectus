---
id: CSP-333
title: Module scaffold + dep-surface enforcement
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 208000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/viewer/` laid down with `mod.rs`, `model.rs`,
`parser/{mod, claude_code, codex, opencode}.rs`, `widget.rs`,
`state.rs`, `input.rs`, `render.rs`, `theme.rs`. All bodies
are stubs returning `ParseError::Malformed` (parsers) or empty
placeholders pending `CSP-334 .. 006`. `mod.rs`
carries `ALLOWED_EXTERNAL_DEPS` and `ALLOWED_BINARY_DEPS`
consts that mirror `docs/transcript-viewer-deps.md`. The
`dep_surface_matches_doc_manifest` test parses the doc's
Markdown tables and asserts the two agree on both the
library surface (11 crates) and the binary-only surface
(`clap`); manually drifting either side fails the test.
`crate::tui::theme::Theme` is re-exported through
`src/viewer/theme.rs` per ADR 0052's tracked carve-out.
The conspectus TUI does not yet route `T` into the new
module — the existing escape-hatch `ClaudeHistoryViewer`
still owns the keybind until `CSP-340`. Eight
viewer-module tests pass; full nextest suite (1141) green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-001`
