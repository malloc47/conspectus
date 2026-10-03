---
id: CSP-440
title: Add rkyv archive derives to the graph model
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 499000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: `graph_snapshot_rkyv_round_trip_preserves_every_field`
  builds a populated snapshot with one of every `NodeKind`,
  one resolved link, one unresolved endpoint, a diagnostic,
  an alias, a pin (with a `Bound` binding), and two
  `node_provenance` entries, then asserts the snapshot
  survives the `rkyv::to_bytes` → `rkyv::from_bytes` cycle
  byte-for-byte after `canonicalize`.
  `metadata_with_every_value_variant_round_trips` covers
  every `Value` shape (String, Int, Float, Bool, Null,
  Array, Object) through the `MetadataAsJson` adapter as a
  regression net.
  `every_node_id_variant_archives_and_round_trips` walks
  each `NodeId` variant through the archive cycle so a
  future variant-payload regression surfaces in CI.
  Full suite green via `cargo nextest run --all-targets
  --all-features` (1761 tests pass); `cargo fmt -- --check`
  and `cargo clippy --all-targets --all-features --
  -D warnings` both clean.
- Notes: rkyv 0.8.16 + bytecheck 0.8.2 added to the
  dependency tree (~13 new transitive crates, all Rust, no
  C bindings). `memmap2` lands in CSP-441 alongside the
  format module.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every model type reachable from `GraphSnapshot`
now carries `#[derive(rkyv::Archive, rkyv::Serialize,
rkyv::Deserialize)]`, including the 9 ID types, the 9
typed `*Node` structs, `GraphLink`, `LinkEndpoint`,
`UnresolvedEndpoint`, `LinkState`, `SourceMetadata`,
`NodeProvenance`, `ResolvedRelationship`, `Diagnostic`,
`PinLastSession`, `PinCandidate`, `PinBinding`,
`PinMuxRef`, and `GraphSnapshot` itself. The
`crate::aliases::AliasOverlay` carried inside
`GraphSnapshot.aliases` gains the same derives so the
archive is whole-snapshot. The `simple_id!` macro picked
up the derive triplet plus the
`#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]`
archive-side compare attribute; the four explicit ID
structs (`CheckoutId`, `AgentSessionId`, `BranchId`,
`ForgePrId`) and the `NodeId` enum gained the same
attribute so the archived form qualifies as a
`BTreeMap` key in `GraphSnapshot.node_provenance` and
`AliasOverlay.entries`.
`serde_json::Value` is handled per the ADR 0083 amendment
via a `MetadataAsJson` adapter in
`src/model/rkyv_adapters.rs` that encodes the whole
`Metadata = BTreeMap<String, serde_json::Value>` field as
a single JSON-text `String` in the archive. The adapter
applies via `#[rkyv(with = MetadataAsJson)]` on
`SourceMetadata.fields` and `UnresolvedEndpoint.metadata`;
no producer or consumer call site changed.
`Cargo.toml` gains `rkyv = { version = "0.8", features =
["bytecheck", "alloc"] }`. `memmap2` and the snapshot
format module land in CSP-441.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-003`
