---
id: CSP-850
title: 'Pi: let the operator name the tools that count as subagents'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 12:05'
updated_date: '2026-11-14 00:19'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 935000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Pi extension counts a session's subagents as the in-flight calls of tools whose name is in a fixed set, `{"subagent"}`, the name of Pi's reference subagent example (ADR 0206 decision 25). Pi has no subagent concept of its own, so a third-party subagent package with other tool names shows its children as ordinary tool calls. Research note 04 (open question 3) asks whether the set should be configurable in Conspectus's config and passed to the extension, through an environment variable or a regenerated extension file; either choice affects how `hook status pi` tells an installed copy from an outdated one, since status compares the file byte for byte. Found while building the Pi slice (CSP-817).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An operator adds tool names the Pi extension counts as subagent calls by listing them, comma-separated, in CONSPECTUS_PI_SUBAGENT_TOOLS where Pi starts, without editing the installed file
- [x] #2 The mechanism, and what hook status pi reports for a configured copy, are recorded in an ADR and in docs/operations.md
- [x] #3 The extension's node tests and the shared event scripts cover a configured name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Extension: read CONSPECTUS_PI_SUBAGENT_TOOLS (comma-separated tool names) once per load, add the names to the default {subagent}, pass the set to the state machine; names are compared exactly and never sent. Bump EXTENSION_VERSION to 3 so hook status pi calls a version-2 copy outdated.
2. Tests: node unit test of the parser, a default-export test with the variable set, and a shared event script (subagent-configured) whose header carries the variable's value; the Rust sequence tests replay it.
3. Rust: hook status keeps comparing the file byte for byte (a configured copy is the same file); adjust the version constants and tests.
4. ADR (environment variable, why not a config key or a regenerated file, what hook status pi reports), operations guide, plugin README, CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Chose an environment variable (CONSPECTUS_PI_SUBAGENT_TOOLS) over a config key written into the installed file: the file stays the embedded copy, so hook status pi keeps its byte comparison and an install stays idempotent; ADR 0227 records the alternatives. Extension bumped to version 3 (a version 2 copy reads outdated, still reports live state).
Validation: just pi-plugin-test 34/34 (new: parser unit test, default export with the variable set, subagent-configured script); Rust replay test tools_the_operator_names_count_as_subagents; cli_smoke hook status pi covers a version 2 copy; checks.sh green; just check 5682 passed, 1 skipped.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Pi extension (now version 3) reads `CONSPECTUS_PI_SUBAGENT_TOOLS`, a comma-separated list of tool names, once when Pi loads it, and counts calls of those tools in flight as subagents beside the reference `subagent` tool, by exact name; the names never leave Pi. The installed file does not depend on the names, so `hook status pi` still compares it byte for byte and reports a configured copy `installed`; a version 2 copy reads `outdated` and still reports live state. ADR 0227 records the mechanism and why not a config key or a regenerated file; docs/operations.md (Pi subagent tools), the extension README, the fixture README, and CHANGELOG describe it.

Verified with `just pi-plugin-test` (34 pass: parser, default export with the variable set, shared script `subagent-configured`), the Rust replay of that script, the `hook status pi` smoke test for a version 2 copy, `/tmp/csp-fu/checks.sh`, and `just check` (5682 passed, 1 skipped). Nothing visible in the TUI changes.
<!-- SECTION:FINAL_SUMMARY:END -->
