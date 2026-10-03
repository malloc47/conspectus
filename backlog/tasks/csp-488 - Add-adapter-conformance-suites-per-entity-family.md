---
id: CSP-488
title: Add adapter conformance suites per entity family
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies: []
ordinal: 170000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New `tests/adapter_conformance.rs`
  integration test with 12 per-family invariant tests:
  * **Harness family (3):** unique `harness_key`s across
    registered adapters, non-empty `display_label`,
    `runtime_signature.harness_key == harness_key()`.
  * **Mux backend family (3):** every impl's
    `backend_key()` is in `KNOWN_MUX_BACKENDS`; every
    `KNOWN_MUX_BACKENDS` entry maps to a
    `ProviderDescriptor` with `ProviderClass::Mux`;
    `KNOWN_MUX_BACKENDS` keys are unique.
  * **Forge family (2):** `GitHubForgeProvider` and
    `GitLabForgeProvider` return distinct `provider()`
    strings; `claims_remote_url` partitions between
    `github.com` / `gitlab.com` / other URLs.
  * **Orchestrator family (2):** registry keys are
    unique; each descriptor resolves a default root when
    `HOME` is set and the disable env var is unset.
  * **Cross-family (2):** `providers::REGISTRY` keys are
    unique; `Mux`-class registry entries and
    `KNOWN_MUX_BACKENDS` agree on membership.
  All 12 pass. Fixture-corpus integration (adapters
  joining the fixture corpus by adding fixtures only) is
  deferred as documented in the story — the corpus loader
  is already fixture-driven, so a new adapter shipping
  fixtures under `tests/fixtures/<key>/` inherits the
  coverage without a loader edit.
- Blockers: first landed chunk of each of Phases B, C, D
  (all landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-016`
