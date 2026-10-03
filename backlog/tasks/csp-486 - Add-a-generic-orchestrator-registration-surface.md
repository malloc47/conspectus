---
id: CSP-486
title: Add a generic orchestrator registration surface
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-473
ordinal: 168000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New `discovery/orchestrator.rs` module
  carries `OrchestratorDescriptor { key, env_root_var,
  env_disable_var, home_relative_default, build:
  fn(PathBuf) -> Box<dyn DiscoveryProvider> }` +
  `pub const REGISTRY: &[OrchestratorDescriptor]` with
  `agent_deck` as the single entry. Adding dmux / herdr /
  pertmux / workmux is a matter of pushing another entry —
  the discovery driver, config table parser, and env-var
  walk pick it up automatically.
  Descriptor accessor `resolve_default_root()` walks
  `env_disable_var → env_root_var → home_relative_default`
  (matches the pre-H-EXT-014 semantics for agent_deck).
  `descriptor_by_key(key)` exposes registry lookup.
  `LocalDiscoveryConfig.agent_deck_root: Option<PathBuf>`
  field removed. Replaced with
  `orchestrator_roots: BTreeMap<String, PathBuf>`. New
  generic builders `with_orchestrator_root(key, root)` and
  `without_orchestrator(key)`; deprecated
  `with_agent_deck_root` / `without_agent_deck` retained as
  aliases so pre-H-EXT-014 test call sites compile
  unchanged.
  `from_env` walks `REGISTRY`, materializes each
  descriptor's default root, and populates
  `orchestrator_roots`. `discover_local_warm_with` iterates
  `REGISTRY` and calls each descriptor's `build` on the
  configured root; the pre-H-EXT-014 hardcoded
  `AgentDeckDiscovery::new(root)` special case is gone.
  New blanket `impl DiscoveryProvider for Box<dyn DiscoveryProvider>`
  so descriptor `build` fns can return an owned box that
  the `LocalDiscovery::with_keyed_provider` API accepts.
  2 new registry tests
  (`agent_deck_descriptor_matches_pre_h_ext_014_env_semantics`,
  `unknown_key_returns_none`). Retired
  `default_agent_deck_root` helper (its logic now lives on
  `OrchestratorDescriptor::resolve_default_root`).
  Deferred: `[orchestrators.<key>]` TOML config-table parse
  surface (env-var contract stays the source of truth for
  now; landing a config-table parser without a second
  orchestrator to exercise it wouldn't be well-motivated).
  ADR 0060 amendment (recording the revisit + what would
  trigger a capability trait) also deferred; the shape here
  matches ADR 0060's original stance so an amendment isn't
  load-bearing until CSP-487 lands its first mutation
  capability. All 25 suites (1527 lib tests: +2 registry
  tests) pass; fmt / clippy clean.
- Blockers: `CSP-473` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-014`
