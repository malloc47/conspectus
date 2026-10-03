---
id: CSP-212
title: Add `tui-markdown` dependency
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 200000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tui-markdown = { version = "0.3",
default-features = false }` added to `Cargo.toml` per ADR
0051. The `highlight-code` feature stays off so `syntect`
and the secondary `ansi-to-tui` path don't land in the dep
graph. Pre-listed in `ALLOWED_EXTERNAL_DEPS` from the
CSP-333 scaffold, so the
`dep_surface_matches_doc_manifest` test passes unchanged.
First use lands with CSP-338's `render_turn`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TRANSCRIPT-008`
