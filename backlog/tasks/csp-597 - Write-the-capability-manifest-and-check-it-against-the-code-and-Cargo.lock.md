---
id: CSP-597
title: Write the capability manifest and check it against the code and Cargo.lock
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 03:24'
updated_date: '2026-10-13 03:27'
labels:
  - h-caps
milestone: m-11
dependencies:
  - CSP-596
priority: high
ordinal: 646000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Scope (ADR 0117): `docs/capabilities.toml` lists programs (with subcommand patterns by context: read, act, hook, dev), socket families and paths, signals, write paths, and a denylist of networking crates. Each section cites its ADRs.

Tests:
- `exec::Program` and `[programs]` list the same programs;
- every cited ADR exists;
- no denied crate is in `Cargo.lock`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 docs/capabilities.toml documents every capability with its ADRs
- [x] #2 Program and manifest drift, a missing ADR, or a networking crate fails a test
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in PR #1. `docs/capabilities.toml` covers 9 programs with read/act/hook/dev subcommand patterns, plus AF_UNIX-only sockets, the daemon socket path, SIGTERM and 0 for teardown, write paths by context, and 24 denied networking crates. Every section cites existing ADRs. The static tests check Program/manifest parity, the ADR citations, and `Cargo.lock` (no denied crate present today).
<!-- SECTION:FINAL_SUMMARY:END -->
