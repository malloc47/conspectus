---
id: CSP-074
title: Record an ADR for Conspectus distribution
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-073
ordinal: 69000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: decide whether external consumers (Atelier today, possibly
  other tools later) depend on Conspectus via crates.io, a pinned
  git revision, a path dependency, or all three. Capture the
  versioning policy, MSRV story, and release cadence. The decision
  must be compatible with the dev-shell's Nix toolchain pinning.
- Tests: docs-only.
- Manual checks: confirm any chosen distribution channel works
  against the Phase 6 dev-shell.
- Blockers: `CSP-073`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted ADR 0016, which makes crates.io the intended
steady-state distribution channel, allows pinned git revisions for
Atelier migration and release validation, limits path dependencies
to local development, and ties the effective MSRV to the stable
toolchain validated by the Nix dev shell.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-002`
