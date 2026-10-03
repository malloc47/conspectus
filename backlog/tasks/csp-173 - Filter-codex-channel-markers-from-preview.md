---
id: CSP-173
title: Filter codex channel markers from preview
status: Done
assignee: []
created_date: '2026-05-19 12:05'
labels:
  - h-preview
milestone: m-11
dependencies: []
ordinal: 191000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/discovery/harness/codex.rs` gained
`apply_codex_channel_marker_filter`, a conservative stripper
that recognizes leading `<turn_aborted>` and `<proposed_plan>`
tags (the constants live in `CODEX_CHANNEL_MARKERS`). When the
tag wraps non-empty content, the prefix (and a matching closing
tag if present) is stripped and the body becomes the preview.
When the body is empty after stripping — a bare
`<turn_aborted>` with nothing after — the filter returns
`None` so the extractor's outer backward walk picks an earlier
message instead. Unknown XML-shaped tags are left verbatim, so
legitimate `<html>` or `<foo>` in user content survives.
Live verification on this workspace: a row that previously
rendered `<turn_aborted> The user interrupted…` now shows the
interrupt text without the marker, and `<proposed_plan>
# Atelier Profiles V1…` becomes `# Atelier Profiles V1…`.
Four new unit tests cover bare-marker skip, marker+body strip
for both known tags, and unknown-tag verbatim preservation.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-006`
