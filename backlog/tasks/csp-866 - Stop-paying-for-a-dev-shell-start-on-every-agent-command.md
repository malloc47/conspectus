---
id: CSP-866
title: Stop paying for a dev-shell start on every agent command
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 03:55'
updated_date: '2026-11-13 23:45'
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
- [x] #1 In an agent session started the documented way, cargo, just, nextest, and backlog run from a worktree without a per-command dev-shell start, and a command reaches cargo in under 1 s
- [x] #2 The toolchain those commands use is the one the flake pins, and a change to `flake.lock` or `rust-toolchain.toml` reaches the next session by the documented steps
- [x] #3 An ADR records the chosen approach and the alternatives weighed, before any new tool or checked-in file lands
- [ ] #4 AGENTS.md, and the README development section if it describes entering the dev shell, tell an agent how to run the checks under the chosen approach
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
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

Approaches weighed in ADR 0226 before the script landed (same commit): starting the agent inside the dev shell (needs every session started that way, outside the repository, and fixes the environment for the session); direnv + nix-direnv with a checked-in `.envrc` (direnv loads from an interactive prompt hook, which agent commands never run, so each command would still need `direnv exec`; neither program is installed here, both would be outside the repository); `nix develop --profile` reused by hand (still starts Nix and the hook, still leaves temp dirs); a harness session-start hook (ties the fix to one harness and its configuration, which is the operator's to choose); host rustup (a second toolchain source, retired by ADR 0110). Chosen: a cached `nix print-dev-env --profile`, loaded by `scripts/dev-env.sh`.
Verified: `scripts/dev-env.sh cargo --version` 0.04 to 0.05 s warm (just 0.03 s, cargo nextest 0.05 s, rustc 0.04 s, backlog 0.36 s, its own start-up), against 7.2 to 8.7 s for `nix develop --command cargo --version` today (one pair at 1.6 s right after the same tree had been evaluated); first build of the copy 7.9 s. The tools resolve to the same store paths as under `nix develop` (rust-default-1.99.0, just-1.50.0, cargo-nextest-0.9.133, backlog-1.53.0); without the script, this machine's PATH finds cargo 1.95.0 in the user's own profile. A test build right after one under `nix develop` was fresh (0.17 s), so the environments match for cargo. A comment added to rust-toolchain.toml rebuilt the copy on the next call (14.6 s) and reverting it rebuilt the original (9.6 s); older copies were pruned. Nested calls do not reload (PATH holds the toolchain once); sourced mode works and leaves no helper functions or variables; it works from any directory; no argument prints the usage and exits 2. No `nix-shell.*` or `nix-develop-*` directory was created by any of it, nor by a whole `scripts/dev-env.sh just check` (passed, 106 s, no edit).
Checks: run-tests.sh 5,681 passed, 0 failed; checks.sh clean; `git diff --check` clean.
WAITS ON THE OPERATOR (AC #4 for AGENTS.md, DoD #2): AGENTS.md is the agent's own instruction file, not edited on another agent's request. Proposed: after "Use `nix develop` for local development. The shell provides Rust tooling and standard checks." add: "An agent, whose every command starts a fresh shell, runs tools through `scripts/dev-env.sh` instead (`scripts/dev-env.sh just check`, or `. scripts/dev-env.sh && ...` for a command line that runs several): it loads the dev shell's environment from a cache that follows `flake.nix`, `flake.lock`, and `rust-toolchain.toml`. Don't wrap commands in `nix develop --command`." The whole patch (CSP-864, 865, 866): /tmp/csp-fu/agents-md.patch, `git apply` from the worktree.
OPTIONAL OPERATOR STEP (not needed for the ACs): start sessions inside the environment, once the script is on main: `nono run <your usual --allow flags> -- /home/jwaggoner/src/oss/conspectus/scripts/dev-env.sh claude`. Commands then need no prefix; the environment is fixed for the session, and the prefix still picks up a flake change mid-session. Untested here (agents may not start `claude`): check that `command -v cargo` in a new session prints the rust-default-1.99.0 store path.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`scripts/dev-env.sh COMMAND` (or `. scripts/dev-env.sh`) runs commands in the dev shell's environment without starting the dev shell: it keeps `nix print-dev-env --profile` output under `target/dev-env/`, keyed by flake.nix, flake.lock, rust-toolchain.toml, the script, and the system, rebuilds it on the first call after any of them changes, and keeps its store paths alive through the profile link. It leaves out the per-call temp dir and the banner hook. ADR 0226 weighs the alternatives (agent started in the dev shell, nix-direnv with `.envrc`, a reused profile, a harness hook, host rustup) and records the choice; flake.nix notes beside the hook that the script skips it; the README tells how to use it.

Measured: a command reaches cargo in 0.04 to 0.05 s (backlog 0.36 s) instead of 7.2 to 8.7 s for `nix develop --command`, with the same store paths for rustc 1.99.0, just, nextest, and backlog, a fresh cargo build across the switch, and no directory left in /tmp; a whole `scripts/dev-env.sh just check` passed. Verified the key change (a rust-toolchain.toml edit rebuilt the copy), nesting, sourced mode, and the full suite and checks.

Open: the AGENTS.md instruction (AC #4) waits on the operator (/tmp/csp-fu/agents-md.patch). Optional operator step: start sessions with `nono run <flags> -- <repo>/scripts/dev-env.sh claude`.
<!-- SECTION:FINAL_SUMMARY:END -->
