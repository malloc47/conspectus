---
id: CSP-683
title: Add launch profiles with `atelier exec` discovery
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 18:13'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-672
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0098-pin-resume-preserves-launch-argv.md
priority: medium
type: feature
ordinal: 741000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Twelve of the operator's 15 pins carry the same bypass flag per harness, and some launches run through `atelier exec`. Slot overrides and recorded argv keep that for rebuilds, but a new `mux launch` starts from the bare harness default. Launch profiles make "how to start this harness" an explicit preference.

Interpretation to confirm when picking this up: "discovery of `atelier exec`" means that when a launch's cwd is inside a discovered Atelier workspace and `atelier` is on `PATH`, the default argv wraps the harness in `atelier exec`. That layer sits below explicit profiles. Harness detection already sees through the wrapper (`adapter_in_command`), and resume splicing preserves it (ADR 0098).

- Scope: `[launch.<harness>] argv` in the user config, overridable by a project's `.conspectus.toml`; wrapper prefixes allowed; a discovered `atelier exec` layer; precedence slot override > recorded > project profile > user profile > discovered `atelier exec` > harness default; the mux launch form and rebuilds without recorded argv use it.
- Tests: precedence tests across every layer; tests that resume splicing works through profile wrappers.
- Manual checks: launch a claude mux inside an Atelier workspace with and without a profile.
- Blockers: the rebuild engine ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Launch profiles in user and project config set the default argv per harness, wrappers included
- [x] #2 Inside an Atelier workspace with `atelier` on `PATH`, the default argv wraps the harness in `atelier exec` unless a profile says otherwise
- [x] #3 Argv precedence matches the rebuild ADR, with tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Launch profiles ([launch.<harness>] argv in user and project config, project winning), atelier exec discovery (atelier on PATH and cwd inside an atelier.toml workspace), and the full precedence slot override > recorded > project profile > user profile > atelier exec > harness default, used by rebuild, mux launch, and the TUI launch form (which shows the source). Verified by launch_default_tests, rebuild mux_tests (including resume splicing through a profile wrapper), and the capability tests.
<!-- SECTION:FINAL_SUMMARY:END -->
