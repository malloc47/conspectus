---
id: CSP-473
title: Add a provider descriptor registry
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies: []
ordinal: 155000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03 (metadata-only scope; ADR 0088).
  `discovery/providers.rs` now owns a
  `ProviderDescriptor { key, kind }` table with 15 entries
  (11 heavy + 4 mutator). `ProviderClass` moves to the same
  module (inherent methods stay in `cache.rs` to keep the
  `ServerIntervals` dependency localized). `cache::provider_class`
  becomes a one-line delegate; `cache::mutator_providers()`
  is derived from the registry; `MUTATOR_PROVIDERS` stays as
  a `&[&str]` alias for slice-indexing callers and is
  pinned to the registry-derived list by a new
  `mutator_const_matches_registry` test. Descriptor
  round-trip and constant-agreement tests added; the
  pre-H-REF-009 `canonical_strings_are_stable` pin is
  unchanged.
  Constructor callback + env-var opt-out plumbing stay on
  `LocalDiscoveryConfig` — deferred to the per-entity
  CSP-474/CSP-476/CSP-480/CSP-484/CSP-486 stories, which extend the
  descriptor with a family-specific adapter reference once
  the trait shape is decided. ADR 0088 records the
  registration convention and what's deferred.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-001`
