---
id: CSP-484
title: Wire forges as an adapter list
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-473
ordinal: 166000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03 (partial). `LocalDiscoveryConfig.forge_runner:
  Option<Box<dyn GhRunner>>` migrates to
  `forge_adapters: Vec<Box<dyn ForgeAdapter>>`. Builders:
  `with_forge_adapter(adapter)` pushes; `with_forge_runner(runner)`
  retained as a deprecated alias that wraps the runner in a
  `GitHubForgeProvider` before pushing (so existing
  dev_scenarios / integration tests compile). `without_forge()`
  clears; `CONSPECTUS_DISABLE_FORGE` env var still zeroes the
  list.
  `ForgeAdapter` trait gains
  `fn claims_remote_url(&self, remote_url: &str) -> bool` with
  a `false` default. `GitHubForgeProvider` overrides to match
  `github.com` substrings on both SSH and HTTPS shapes; also
  implements `ForgeAdapter` (in addition to its existing
  `DiscoveryProvider` impl) so it can be registered in the
  `forge_adapters` list.
  `discover_local_warm_with` drains `config.forge_adapters` and
  hands them to `forge::ForgeDiscovery` via a new
  `with_boxed_adapter` builder — the pre-H-EXT-012 direct
  `GitHubForgeProvider` construction bypass is gone.
  Deferred (docs-only): moving `GhRunner` / `SystemGh` /
  `FakeGh` / `GhOutcome` types from `forge/mod.rs` into
  `forge/github.rs` — those files stay put in this pass
  because ~10 callers import them via `forge::GhRunner` and
  the mechanical rename is orthogonal to the trait-shape work
  here. Land alongside CSP-485's GitLab adapter which will
  force the same type-move-with-re-export shape for `GlabRunner`.
  All 25 suites (1521 lib tests) pass; fmt / clippy clean.
- Blockers: `CSP-473` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-012`
