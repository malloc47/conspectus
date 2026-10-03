---
id: CSP-485
title: Add a second forge adapter (GitLab or Gitea)
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-484
  - CSP-108
ordinal: 167000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03 as a **skeleton**. New
  `discovery/forge/gitlab.rs` module carries
  `GitLabForgeProvider` implementing both `ForgeAdapter`
  and `DiscoveryProvider`; `claims_remote_url` matches
  `gitlab.com` substrings on HTTPS and SSH URL shapes.
  `discover` returns an empty `GraphFragment` because
  **`CSP-108` blocks real gitlab discovery** — the
  multi-forge `ForgePr` identity model must land first, and
  the ADR for that is out of scope for CSP-485 alone.
  Wiring: gitlab adapter is opt-in via
  `CONSPECTUS_ENABLE_GITLAB` in `from_env` (default off so
  the stub doesn't cluster warm-start caches with a spurious
  `gitlab` provider slice). No `GlabRunner` trait /
  `SystemGlab` impl in this pass — those land alongside the
  real discovery implementation when the identity model
  settles.
  Tests:
  * `discovery/forge/gitlab.rs::tests` — 4 tests covering
    HTTPS / SSH URL claims, provider-string uniqueness,
    empty-fragment discovery.
  * `discovery/forge/mod.rs::tests` — 2 new tests
    exercising the CSP-484 seam with two fake adapters:
    `claims_remote_url_partitions_two_adapters_by_host`
    pins the routing shape; `forge_discovery_accepts_two_adapters_via_boxed_registration`
    confirms the `with_boxed_adapter` builder handles
    heterogeneous `Box<dyn ForgeAdapter>` entries the way
    `LocalDiscoveryConfig::forge_adapters` flows through.
  Acceptance test for CSP-484 satisfied: zero edits
  outside the new module + the env-var registration + one
  line in `LocalDiscoveryConfig::from_env`.
  All 25 suites (1525 lib tests: +4 gitlab + +2 routing)
  pass; fmt / clippy clean.
- Blockers: `CSP-484` (landed), `CSP-108`
  (still open; blocks the real discovery implementation
  but not the skeleton that proves the routing shape).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-013`
