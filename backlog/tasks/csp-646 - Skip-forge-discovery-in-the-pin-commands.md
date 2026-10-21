---
id: CSP-646
title: Skip forge discovery in the pin commands
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 15:15'
updated_date: '2026-10-21 15:22'
labels:
  - pins
  - forge
dependencies: []
priority: high
type: bug
ordinal: 704000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0133 removed the watch window and the pre-attach refresh from a TUI pin launch, but the operator saw no speed-up. The launch subprocess (`conspectus pin launch <id> --no-attach`) starts with a full local discovery, and a one-shot command has no forge listings cached (ADR 0116), so it runs `gh pr list` twice per GitHub remote before it touches tmux. On the operator machine that is about 13 s with enterprise hosts queried (ADR 0128), against 0.6 s with forge disabled; the steps ADR 0133 removed were about 1 s. Pin resolution reads harness, mux, git, and process state, never PRs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The pin commands (launch, attach, list, show, and the edit commands that resolve pins) run discovery without forge adapters
- [x] #2 Pin binding, resume, and launch decisions are unchanged
- [x] #3 Commands that may need PR candidates (declared confirm/ignore, table prs, graph) still run forge discovery
- [x] #4 A test shows the pin discovery config has no forge adapters; the measured pin list time drops to the no-forge figure; an ADR records the change
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The pin commands now discover with every provider except the forge adapters (`pin_discovery_config`, ADR 0135), so `conspectus pin launch --no-attach`, the subprocess a TUI pin launch runs, no longer spends a one-shot round of `gh pr list` calls before it reaches tmux. Measured `pin list` (same discovery) on the operator machine: 12.9-13.7 s before, 0.54-0.63 s after, with the same binding states. Other commands keep forge discovery. Verified with fmt, clippy `-D warnings`, and the full nextest suite (2534 passed, including a new test that the pin discovery config has no forge adapters). An end-to-end TUI launch was not timed in this session.
<!-- SECTION:FINAL_SUMMARY:END -->
