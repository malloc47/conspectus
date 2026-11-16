---
id: CSP-890
title: Cap and manage hook trace files
status: To Do
assignee: []
created_date: '2026-11-16 18:02'
updated_date: '2026-11-16 20:02'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-887
priority: low
type: enhancement
ordinal: 966000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The hook trace (ADR 0242, CSP-887) appends one JSON line per hook call to a file under the state directory for as long as the environment variable stays set, and nothing rotates, caps, or collects the files. A trace left on in a long-running session grows with every tool call, and an operator who set the variable once has no way to see what it left behind except by looking in the directory.

Keep it opt-in and operator-initiated, as the ADR requires: the daemon never traces and never cleans up.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A trace file stops growing at a size cap (a default in the low megabytes, with an environment override), and its last line says it was truncated
- [ ] #2 `conspectus hook trace list` shows each trace's name, size, line count, harness mix, and time span, and `conspectus hook trace clear` deletes one named trace or all of them, only on the operator's command
- [ ] #3 Both commands read and delete only under the hook-trace directory of the state directory, appear in docs/capabilities.toml if the manifest needs it, and are described in docs/operations.md next to the recording section
- [ ] #4 Tests cover the cap, list on an empty and a populated directory, and clear leaving other state files alone
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
