---
id: CSP-513
title: TTL-cache `ForgeDiscovery` output (`9a691a3`)
status: Done
assignee: []
created_date: '2026-07-29 02:07'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 537000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Empty PR results produced no `github` provenance stamps → freshness gate never marked forge fresh → every class cycle respawned `gh pr list`. Same failure mode later addressed for tmux/zellij in 011. A `provider_last_run` gate refactor would replace both TTL caches with one correct fix; deferred.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-005`
