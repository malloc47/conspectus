---
id: CSP-866
title: Stop paying for a dev-shell start on every agent command
status: To Do
assignee: []
created_date: '2026-11-13 03:55'
labels:
  - h-devloop
milestone: m-11
dependencies: []
references:
  - flake.nix
  - AGENTS.md
  - docs/adr/0010-task-runner-selection.md
priority: high
type: chore
ordinal: 896000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Outside the dev shell an agent session has no `rustc`, so agents wrap each command in `nix develop --command ...`. Each wrap evaluates the flake and runs the shell hook: 7.9 s per invocation with a warm store, measured 2026-11-12.

In this project's agent transcripts from 2026-10-21 to 2026-11-12, 88% of the 3,128 cargo commands were wrapped this way, about 6 hours of shell start-up in three weeks. The median wrapped command took about 40 s, so the wrap adds about a fifth. The dev machine's `/tmp` also holds about 8,000 empty `nix-develop-*` directories that these starts left behind, 1,044 of them from 2026-11-12 alone.

Approaches to weigh include starting the agent from inside the dev shell, nix-direnv with a checked-in `.envrc`, and a cached `nix print-dev-env` profile. AGENTS.md requires an ADR before a new tool or checked-in convention is adopted.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In an agent session started the documented way, cargo, just, nextest, and backlog run from a worktree without a per-command dev-shell start, and a command reaches cargo in under 1 s
- [ ] #2 The toolchain those commands use is the one the flake pins, and a change to `flake.lock` or `rust-toolchain.toml` reaches the next session by the documented steps
- [ ] #3 An ADR records the chosen approach and the alternatives weighed, before any new tool or checked-in file lands
- [ ] #4 AGENTS.md, and the README development section if it describes entering the dev shell, tell an agent how to run the checks under the chosen approach
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
