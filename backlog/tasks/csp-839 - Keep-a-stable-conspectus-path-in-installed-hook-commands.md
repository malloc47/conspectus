---
id: CSP-839
title: Keep a stable conspectus path in installed hook commands
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 06:05'
updated_date: '2026-11-15 00:08'
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
- [x] #1 When the running binary is in the Nix store or a cargo build directory, hook init claude-code|codex installs the first conspectus on PATH, else in a Nix profile, that is the same binary (same canonical path or content) and does not itself sit in one; otherwise it installs the running binary's path and warns why; a binary elsewhere is installed as it is
- [x] #2 hook init prints the program the hooks run and why whenever it chose it, and without --command moves installed entries whose program is gone (or that run a fragile path once a stable one runs this build) to that choice
- [x] #3 hook status reports on stderr, per harness, an installed hook whose program no longer exists (Claude Code and Codex entries; the Pi extension and opencode plugin's CONSPECTUS_HOOK_BIN or PATH conspectus) and names the fix, with its stdout lines unchanged
- [x] #4 Tests cover a store-path binary, a target/ build, a PATH name that resolves elsewhere, and a missing program, in scratch homes
- [x] #5 The rule is recorded in an ADR, and docs/operations.md and CHANGELOG.md describe it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `src/cli/hook_program.rs`: `choose(exe, env)` keeps the running binary when it sits where it stays; when it is in the Nix store (`/nix/store` or `$NIX_STORE_DIR`) or a cargo build directory (`target/debug|release`, or under a `CACHEDIR.TAG`), it takes the first `conspectus` on PATH, then in a Nix profile (`~/.nix-profile`, `~/.local/state/nix/profiles/profile`, `/etc/profiles/per-user/$USER`, `/run/current-system/sw`), whose own path is not fragile and that is the same binary (same canonical path or same content), recorded by its absolute (link) path; else the running binary with a warning saying why. Also `program_of` (a command line's first shell word) and `standing` (found, gone, or not on this PATH).
2. `hook init claude-code|codex`: prints which program the hooks run and why when Conspectus chose it; without `--command`, moves installed entries whose program is gone, or that run a fragile path once a path that stays runs this build, to the chosen command (as `--command` would); otherwise keeps them as today.
3. `hook status`: per harness, a stderr note when the installed program is gone (Claude Code, Codex: from the entries; Pi, opencode: `$CONSPECTUS_HOOK_BIN`, else `conspectus` on PATH), naming the fix. Labels and stdout unchanged.
4. Tests: unit tests in scratch dirs (store path to PATH link, PATH name resolving elsewhere to profile link, same-content copy, store PATH entry skipped, target build, custom target dir, fallback warning, parsing); integration tests in scratch homes running a hard-linked copy of the binary from a fake store with a profile link, and a missing program reported by status and repaired by init, for Claude Code, Codex, and Pi.
5. ADR 0240, operations.md (install and troubleshooting), CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Choices to review: (1) re-running `hook init` without --command now moves installed entries whose program is gone, and also entries that all run a store/build path once a stable PATH or profile link runs this build (the upgrade path for installs made before this change); entries whose program exists elsewhere, and bare names, are kept. (2) The stable candidate is recorded by its own absolute (link) path, not the bare name `conspectus`, since harnesses run hooks with the PATH they started with. (3) `NIX_STORE_DIR` is honored for the store location (Nix itself honors it; it also lets tests use a fake store). (4) The Pi extension and opencode plugin embed no path, so they need no stable-path choice; `hook status` only says when this shell finds neither `$CONSPECTUS_HOOK_BIN` nor `conspectus` on PATH. Stdout lines of `hook status` are unchanged; notes go to stderr.
Verified: 11 unit tests in src/cli/hook_program_tests.rs (store path to PATH link, PATH name of another build to profile link, store PATH entry skipped, same-content copy, target build and custom target dir, fallback warning, parsing, quoting); cli_smoke hook_init_installs_a_path_that_stays_for_a_binary_in_the_nix_store (hard-linked binary in a fake store, profile link, PATH name resolving elsewhere, fallback, migration), hook_status_reports_a_hook_whose_program_is_gone_and_init_repairs_it (Claude Code and Codex, target build fallback), hook_status_pi_reports_a_writer_it_cannot_find; two existing tests now use an existing old program; just check: 5801 passed, 1 skipped.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`hook init claude-code|codex` no longer strands the hooks on a Nix store path or a cargo build directory (ADR 0240): when the running binary sits in one, it installs the first `conspectus` on PATH, else in a Nix profile, that is the same build (same canonical path or content) and not itself fragile, by its link's path; otherwise the running path with a stderr warning saying why. It prints which program the hooks run and why. Re-running it moves entries whose program is gone (or that run a fragile path once a stable one runs this build); `--command` still sets the command outright. `hook status` reports on stderr, per harness, a hook program that no longer exists (for the Pi extension and opencode plugin, `$CONSPECTUS_HOOK_BIN` or PATH `conspectus`) and names the fix, with stdout unchanged. New module `src/cli/hook_program.rs`; operations.md gains "Which program the hooks run" and a revised troubleshooting bullet; CHANGELOG under Fixed. Verified by unit tests on scratch directories, scratch-home integration tests running a hard-linked copy of the binary from a fake store, and `just check` (5801 passed, 1 skipped).
<!-- SECTION:FINAL_SUMMARY:END -->
