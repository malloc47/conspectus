---
id: CSP-512
title: >-
  Cache `GitProbe::probe` results across cycles keyed on `.git/HEAD` / `config`
  / `refs/heads/` / `packed-refs` mtimes…
status: Done
assignee: []
created_date: '2026-07-29 02:07'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 536000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Cache `GitProbe::probe` results across cycles keyed on `.git/HEAD` / `config` / `refs/heads/` / `packed-refs` mtimes (`d27a987`). `observed_cwd_git_fragment` otherwise spawned ~150 git subprocesses per harness cycle (~10 spawns × 15 unique cwds).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-004`
