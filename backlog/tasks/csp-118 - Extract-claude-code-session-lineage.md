---
id: CSP-118
title: Extract claude-code session lineage
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-lineage
milestone: m-11
dependencies: []
ordinal: 181000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Resolution: `src/discovery/harness/claude_code.rs` now reads the
  first record's `parentUuid` plus a bounded transcript tail to
  extract the leaf uuid, then matches within each project directory.
  Resolved matches emit a `ParentSession` candidate from child to
  parent `AgentSession`; unresolved parents preserve `parentUuid`
  under `UnresolvedEndpoint`. `lineage_kind` is `"compaction"` when
  the first cross-session record has `type == "summary"` and
  `"resume"` otherwise. Atelier's `lineage_kind` source-metadata
  field was renamed to `lineage_fidelity` per ADR 0018, with the
  new `lineage_kind` carrying the `fork` / `fresh` operation value;
  `harness_mux_snapshots__fork_associated_session_and_unresolved_lineage`
  and the atelier-delegation graph snapshot are updated. Manual
  `~/.claude` validation pending.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-LINEAGE-002`
