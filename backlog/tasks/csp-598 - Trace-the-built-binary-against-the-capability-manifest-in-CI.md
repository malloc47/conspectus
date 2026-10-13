---
id: CSP-598
title: Trace the built binary against the capability manifest in CI
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 03:24'
updated_date: '2026-10-13 03:27'
labels:
  - h-caps
milestone: m-11
dependencies:
  - CSP-597
priority: high
ordinal: 647000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Scope (ADR 0117): `tests/capabilities.rs` runs the binary under `strace -ff` on a materialized showcase world, with a temporary home, temporary XDG directories, and stand-in gh/tmux/zellij/wt scripts. The scenarios are:
- the dev builder;
- read-only commands and the TUI;
- the daemon and its clients;
- operator actions;
- hook install.

It checks the Conspectus process's execs and arguments, sockets, signals, and writes against the manifest by context. CI installs strace and sets `CONSPECTUS_REQUIRE_STRACE`; the nix shell adds strace on Linux.

Tests: a planted `git fetch` and a TCP connect must be reported.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The trace test fails on a program, subcommand, socket, signal, or write the manifest doesn't allow
- [x] #2 CI runs the trace and fails rather than skips without strace
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in PR #1. `tests/capabilities.rs` traces 21 commands across 5 scenarios, from the dev builder to hook install, and checks the Conspectus process's execs and arguments, sockets, signals, and writes by context.

Verified by planting a `git fetch --all` in discovery and a TCP connect in the CLI: both were reported in the read-only and daemon scenarios.

CI installs strace and sets `CONSPECTUS_REQUIRE_STRACE`; on PR #1 the trace ran and passed in 3.4s. The nix shell adds strace on Linux.
<!-- SECTION:FINAL_SUMMARY:END -->
