---
id: CSP-094
title: Add `conspectus node show <id>`
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-obs
milestone: m-11
dependencies: []
ordinal: 111000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New `src/output/node_show.rs` module exposes
`resolve_node_id` (with `NodeResolveError::{NotFound, Ambiguous}`)
and `render_node_show`. The `conspectus node show <id>` subcommand
accepts the short content-addressed prefix from the session table
(CSP-130), the full `NodeId` `Display` form, or the harness/mux
label, and prints the node plus every outgoing/incoming candidate
link (with source-metadata adapter, evidence, and fields), every
resolved relationship touching the node, and every diagnostic
referencing it. Unit tests cover each accepted form, ambiguous
prefixes, and the rendered output shape; four CLI integration
tests exercise the full discovery → resolve → render path
including the round-trip from `session --wide` to `node show`.
`docs/operations.md` documents the new command and the accepted
`<id>` forms.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-OBS-002`
