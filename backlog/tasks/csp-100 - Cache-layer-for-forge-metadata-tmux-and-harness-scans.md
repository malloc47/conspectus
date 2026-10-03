---
id: CSP-100
title: 'Cache layer for forge metadata, tmux, and harness scans'
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-prod
milestone: m-11
dependencies: []
ordinal: 133000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: design.md commits to caches living under `$XDG_DATA_HOME` (and
  keeping them outside project trees), but no cache code exists. Define
  a cache schema, TTL policy, and `--no-cache` / `--refresh` flags.
  Apply first to forge (`gh pr list` per repo is the most expensive
  call) and reuse the seam for tmux and harness state.
- Tests: cache hit/miss, TTL expiry, schema-version mismatch, and
  `--no-cache` flag behavior.
- Blockers: a new ADR for the cache layout and freshness rules.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PROD-002`
