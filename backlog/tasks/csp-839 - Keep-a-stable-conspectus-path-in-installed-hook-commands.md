---
id: CSP-839
title: Keep a stable conspectus path in installed hook commands
status: To Do
assignee: []
created_date: '2026-11-12 06:05'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: medium
type: enhancement
ordinal: 924000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook init` writes the running binary's absolute path (`current_exe()`) into every hook command. When that is a Nix store path or a `cargo run` build, the next upgrade or garbage collection leaves the hooks pointing at a file that no longer exists. With the fourteen Claude Code live-state events installed, every tool call and prompt then runs a command that fails, which Claude Code reports as a hook error each time, and live state silently stops. Research note 01, section 4.2, recommends a stable path; nothing picks one today.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hook init installs a command that survives an upgrade of the binary (for example a stable PATH name or profile link when one resolves to the running binary), and says which it chose
- [ ] #2 hook status reports an installed command whose program no longer exists
- [ ] #3 Tests cover a store-path binary and a missing program in a scratch home
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
