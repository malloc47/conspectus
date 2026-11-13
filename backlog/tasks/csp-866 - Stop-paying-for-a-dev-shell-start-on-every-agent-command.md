---
id: CSP-866
title: Stop paying for a dev-shell start on every agent command
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 03:55'
updated_date: '2026-11-13 23:30'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Weigh the approaches in ADR 0226 before any file lands: start the agent inside the dev shell; nix-direnv with a checked-in `.envrc`; a cached `nix print-dev-env` profile; a harness session-start hook; host rustup. Constraints: every agent command starts a fresh shell, sessions start inside the nono sandbox, prefer nothing outside the repository.
2. Chosen: `scripts/dev-env.sh`, which keeps `nix print-dev-env --profile` output under `target/dev-env/`, keyed by flake.nix, flake.lock, rust-toolchain.toml, the script, and the system; strips the per-call temp dir and the banner hook; runs `COMMAND` or is sourced. A session may also start inside it (operator step, optional).
3. flake.nix: a comment beside the shell hook that the script leaves the hook out.
4. Docs: README development section; the AGENTS.md text goes to the operator as a patch.
5. Verify: time a command that reaches cargo (and just, nextest, backlog) through the script, the toolchain path against `nix develop`, a key change rebuilding the cache, nested calls, sourced mode, no leftover /tmp directories; full suite and checks; commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Prototype of scripts/dev-env.sh works: 0.04 to 0.05 s per command that reaches cargo, just, nextest, or rustc (backlog 0.36 s, its own start-up), against 1.6 to 12 s for nix develop --command; the first call after a change to flake.nix, flake.lock, rust-toolchain.toml, or the script rebuilds the copy in 8 to 15 s.
<!-- SECTION:NOTES:END -->
