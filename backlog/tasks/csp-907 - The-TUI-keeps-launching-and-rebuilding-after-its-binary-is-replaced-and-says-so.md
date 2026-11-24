---
id: CSP-907
title: >-
  The TUI keeps launching and rebuilding after its binary is replaced, and says
  so
status: Done
assignee:
  - '@claude'
created_date: '2026-11-24 21:41'
updated_date: '2026-11-24 21:54'
labels:
  - tui
  - launch
milestone: m-11
dependencies: []
priority: medium
type: bug
ordinal: 982000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Recreating the `maj-chat` pin from a TUI failed with `rebuild of maj-chat failed to spawn: No such file or directory (os error 2)`, and the command record showed `argv: …/target/release/conspectus (deleted) rebuild`. Nothing about the pin was wrong. The TUI runs rebuild, new-mux, and launch by running `conspectus <args>` as a child process (`run_conspectus_subprocess` in `src/tui/runtime/launch.rs`), finding the binary with `std::env::current_exe()`. On Linux that returns the path with ` (deleted)` appended once the file has been replaced, which is what `cargo build --release` does to the binary a running TUI was started from. The child is then asked to run a file that does not exist. Found on 2026-11-24: one of three open TUIs, started at 08:45, before the 11:28 rebuild, was the only one failing.

So after every rebuild of the release binary, any TUI left open stops being able to rebuild a pin, launch a harness, or start a mux, with an error that does not say why or what to do. The fix is to carry on and say so: run something that exists, and warn the operator that the TUI is out of date so they restart it at a convenient moment.

Out of scope: how `hook init` chooses the program it installs (ADR 0240; it runs in a short-lived CLI process), and a TUI talking to a newer daemon's snapshot format (CSP-704).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When the binary a TUI started from has been replaced on disk, rebuild, new mux, and launch still run, using the binary now at that path, instead of failing with `No such file or directory` for a path ending in `(deleted)`
- [x] #2 When nothing is left at that path (a `cargo clean`, a collected store path), they run the image of the running TUI, which is still available, and only when that too cannot run does the failure say the program is gone and to restart the TUI, not a bare `os error 2`
- [x] #3 Whenever either fallback is used, the operator gets a warning in the message log and a toast saying the binary changed under the TUI and to restart it when convenient; the action still reports its own outcome as it does now
- [x] #4 A TUI whose binary is as it started behaves as today, with the same program and no warning
- [x] #5 Tests cover choosing the program for each state (intact, replaced, removed, unknown) and the warning, and that a launch through the fallback really runs; `docs/capabilities.toml` and its tests still pass, and the operator guide and an ADR say which binary runs and why
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `exec::running_binary()` (the one place programs are started, ADR 0117): a pure `locate` over `current_exe()`, the process id, and an `exists` check. The file at the path is there: run it, no fallback. Otherwise if the path ends in ` (deleted)` (what Linux appends once the file is replaced): the path without the marker, if a file is there now (replaced: run the binary that is there, which is also what the restarted daemon runs); else `/proc/<pid>/exe`, the image this process is running (removed). `current_exe()` failing keeps today's `conspectus` on `PATH`. A file whose real name ends in the marker is checked first, so it is not mistaken for a deleted one.
2. `tui/runtime/launch.rs::run_conspectus_subprocess` runs that program. When a fallback was used it posts one warning to the message log and one toast, saying the binary changed under the TUI and to restart it when convenient; the action then reports its own outcome as now. A spawn that still fails `NotFound` gets a reason added saying the binary this TUI started from is gone and to restart it, so the summary the callers build does not end in a bare `os error 2`.
3. Version skew, decided: prefer the file now at the path over the running image, because a restarted daemon and a new CLI agree with each other and an old image reading a newer daemon's snapshot is the hazard CSP-704 describes. The running image is only the last resort, when nothing is at the path.
4. Tests: `locate` for intact, replaced, removed, replaced-by-a-file-named-with-the-marker, and `current_exe` failing; that executing `/proc/<pid>/exe` of a process whose file was deleted runs the real image (a copied `sh` deleted under a live child); the warning text per fallback and none when intact; the NotFound reason. Red-check by making `locate` return `current_exe()` as before.
5. Docs: an ADR (0255) recording the order and the warning; `docs/capabilities.toml`'s `[programs.conspectus]` cites it (the traced test only ever runs an intact binary); the operator guide's note on rebuilding Conspectus while a TUI is open; CHANGELOG.
6. `just check`. Out of scope: `hook init` (ADR 0240) and CSP-704.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Cause, confirmed on the operator's machine and then reproduced: three `csp` TUIs were open, and only the one started at 08:45, before the 11:28 release rebuild, had `/proc/<pid>/exe` ending in `(deleted)`. `run_conspectus_subprocess` (`src/tui/runtime/launch.rs`) ran `current_exe()` verbatim, and the rebuild, new mux, and launch actions all go through it.
Fix: `exec::RunningBinary::locate` picks, in order, the path itself, the file now at the path with the ` (deleted)` marker removed (what a rebuild put there, and what the restarted daemon runs), and the running image at `/proc/<pid>/exe`. The TUI posts a warning to the message log and a toast whenever it had to fall back, and a spawn that still finds nothing says to restart the TUI. ADR 0255 records the order. I chose the new file over the running image because an old image against a newer daemon is the skew CSP-704 describes, so the image is only the last resort.
Verification: `just check` passed (6019 tests, 1 skipped; fmt, clippy, rustdoc -D warnings, diff check; `tuikit-features` skipped, nothing under `crates/tuikit` changed). 14 new tests; with the fix disabled, 8 fail and the 6 that pass guard unchanged cases. One test is real: it deletes a copy of `sh` under a live child and runs its `/proc/<pid>/exe`, so the last-resort path is shown to work on this machine and inside the sandbox, not just assumed. A real-TUI check on a private tmux server (own `TMUX_TMPDIR`, no inherited server, socket verified private before cleanup; the operator's tmux untouched): the baseline (the installed binary) reproduced `mux e2e-mux create failed to spawn: No such file or directory (os error 2)` after its binary was replaced, and this build created the mux and showed the toast `conspectus was replaced while this TUI ran: this used the new one. Restart the TUI when convenient.` The run's `attach failed` line is the harness's: a detached private session has no tmux client to switch.
Not covered end to end: the removed-binary path was exercised by the unit test, not through the real TUI. Not done, by design: nothing tells the operator a TUI is stale before an action needs the binary (ADR 0255, alternatives); a standing indicator would be its own task. A TUI started from a build without this fix is unaffected by it, so the operator's 08:45 TUI still has to be restarted once. Not run locally: CI's `cargo test --workspace`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A TUI left open across a rebuild of Conspectus no longer fails to rebuild a pin, launch, or start a mux. Those actions run `conspectus` again, and the TUI used `current_exe()` verbatim, which reads `<path> (deleted)` once the file is replaced. `exec::RunningBinary::locate` now runs the path if it is there, else the file now at it, else the running image through `/proc/<pid>/exe`; a fallback posts a warning to the message log and a toast saying to restart the TUI when convenient, and a spawn that still finds nothing says so instead of ending in `os error 2`. ADR 0255, the operator guide, the capabilities manifest, and the CHANGELOG updated.
Verified with 14 new tests (8 fail with the fix disabled, including one that runs a deleted binary's real image), `just check` (6019 passed), and the real TUI on a private tmux server: the installed binary reproduced the `(deleted)` spawn failure and this build created the mux and warned. Work is on branch `fix/tui-survives-replaced-binary`, uncommitted.
<!-- SECTION:FINAL_SUMMARY:END -->
