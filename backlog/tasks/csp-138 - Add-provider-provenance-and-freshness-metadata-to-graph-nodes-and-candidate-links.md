---
id: CSP-138
title: >-
  Add provider provenance and freshness metadata to graph nodes and candidate
  links
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 378000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: `merge_fragments_folds_node_provenance_first_write_wins`,
  `node_without_provenance_falls_back_to_schema_defaults`,
  `node_provenance_sidecar_populates_discovery_columns`,
  `link_source_metadata_drives_discovery_columns`. The existing
  `graph_snapshot_field_drift_guard` destructure test picks up
  `node_provenance` so future field additions stay surfaced.
  Snapshot tests across `local_discovery_snapshots`,
  `harness_mux_snapshots`, `declared_snapshots`, `forge_snapshots`,
  and `atelier_delegation_snapshots` regenerated with a shared
  `support::redact_freshness_epoch` helper that normalizes the
  wall-clock-derived epochs to a stable placeholder.
- Follow-ups: `CSP-091` (centralizing provider key constants
  into a `discovery::providers` module) is the next cleanup;
  the strings in this commit match the existing
  `source_metadata.adapter` literals exactly so the CSP-091
  rename is a mechanical pass. ADR 0037's optional
  `provider_state` write is still deferred — `CSP-139` is the
  natural home for that since it owns the save/load lifecycle
  that knows whether a provider ran successfully.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in two commits.

The skeleton (commit 1) added `model::NodeProvenance`,
`GraphSnapshot::node_provenance: BTreeMap<NodeId, NodeProvenance>`
(with a custom serde shape that emits a JSON array since `NodeId`
is structured), `SourceMetadata::freshness_epoch: Option<i64>`,
`GraphFragment::node_provenance`, and updated `merge_fragments`
to fold per-fragment sidecar maps with first-write-wins. The
SQLite loader now writes `discovery_provider` /
`discovery_freshness_epoch` from the sidecar and from
`SourceMetadata`; nodes/links without provenance fall back to the
schema defaults (`'unknown'` / `0`).

The producer instrumentation (commit 2) added two reusable
helpers in `discovery/mod.rs` — `current_epoch()` and
`stamp_fragment(fragment, provider, epoch)` /
`stamp_snapshot_mutations(snapshot, provider, epoch)` — and
called them from every adapter and mutator:
`git`, `git::cwd` (the `observed_cwd_git_fragment` synthesis),
`atelier`, `generic_workspace`, `agent_deck`, `tmux`,
`forge::github` (key `github`), the four harness adapters
(`claude-code`, `codex`, `opencode`, `aider`), `cross_link`,
`codex_log`, `hook_sidecar`, and `declared`. The helpers'
first-write-wins semantics keep the canonical provider attached
to the row that emitted it; later mutators only stamp entries
nobody else owned. The existing `source_metadata.adapter` string
on every link is preserved.

Resolver output (`resolved_relationships`) is not stamped per
ADR 0037's current schema — the table doesn't carry provenance
columns. Aliases / pins are also not stamped; they sit in their
own snapshot sidecars (`snapshot.aliases`, `snapshot.pins`) that
don't participate in node/link eviction.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-002`
