---
id: CSP-529
title: Launch a harness in a fresh mux without persisting a pin (ADR 0096)
status: Done
assignee: []
created_date: '2026-09-30 16:57'
labels:
  - h-mux-launch
milestone: m-18
dependencies: []
ordinal: 554000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: operators today have to persist a pin to spawn a
  harness through Conspectus's launch pipeline, or drop out to a
  shell for a one-off. Neither the pin nor the bare-mux path fits
  "spawn Codex here right now, no dashboard row." ADR 0096 names
  this as `conspectus mux launch <harness>` and the `m`-keyed Mux
  action menu.
- Envelope: ADR 0087 categories 3 (`new-session`) + 4 (Conspectus-
  constructed harness argv). No new mutation category, no pin or
  sidecar write. Worktree toggle reuses ADR 0092 / ADR 0094
  realize-at-launch, idempotent.
- Scope (ships the feature end-to-end; form-primitive extraction is
  the follow-up in CSP-530):
  1. New standalone `MuxLaunchFormState` widget in
     `src/tui/widgets/mux_launch.rs` — harness / cwd / mux name /
     socket / argv / worktree toggle + branch, with cursor +
     validation. Mode fixed at open; no persistence toggle.
  2. Mux action menu overlay in `src/tui/widgets/mux_menu.rs`,
     keyed on `m`, with entries `New tmux session` (opens
     `NewMuxFormState`) and `Launch harness in new mux…`
     (opens `MuxLaunchFormState`). ADR 0095 follow-up
     discharged here.
  3. Wire `Msg::CommitMuxLaunch(MuxLaunchRequest)` +
     `Effect::Exec(ExecSpec::MuxLaunch { … })` in the runtime,
     add `execute_mux_launch` that re-execs into
     `conspectus mux launch … --no-attach` and attaches.
  4. CLI: `conspectus mux launch <harness> --name <mux-name>
     [--cwd] [--socket] [--argv…] [--worktree-branch]
     [--worktree-repo] [--no-attach] [--scan-root]` in
     `src/cli/mux.rs` as a peer of `MuxCommand::New`.
  5. `m` key in the fallback keymap; help overlay lists the
     menu.
- Non-goals: no pin write, no binding persistence, no resume
  splicing (pin-scoped), no `send-keys` seeding, no shared form
  primitive with pin-create (deferred to CSP-530), no
  visual redesign of any existing form.
- Tests: `FakeTmux` new-session call assertion for mux launch
  with harness argv; CLI parse test for `mux launch` including
  worktree flags; snapshot for the new `MuxLaunchFormState`
  form; Mux action menu snapshot with both entries visible;
  keymap regression for `m`.
- Blockers: none. ADR 0096 lands as part of this story.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
**landed as ADR 0096.** `conspectus mux launch
<harness>` in `src/cli/mux.rs` (with `--name`, `--cwd`,
`--socket`, `--argv…`, `--worktree-branch` +
`--worktree-repo`, `--no-attach`, `--scan-root`) plus 9
CLI parse + FakeTmux tests. TUI: `m` opens the
`MuxMenuState` overlay (`src/tui/widgets/mux_menu.rs`) with
`New tmux session` and `Launch harness in new mux…`
entries; the second entry commits `Msg::OpenMuxLaunchForm`
which seeds and opens `MuxLaunchFormState`, built on the
shared `LaunchSpecFormState` primitive (`src/tui/widgets/
launch_spec_form.rs`) introduced in the same commit so
ADR 0097's follow-up migration only touches pin-create.
`Msg::CommitMuxLaunch` → `ExecSpec::MuxLaunch` →
`execute_mux_launch` re-execs the CLI, refreshes discovery,
and attaches through the pin-launch attach path. Help
overlay gains the "Mux (ADRs 0095, 0096)" section covering
`n` and `m`. ADR 0095's `m`-keyed Mux action menu follow-up
discharged.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUX-LAUNCH-001`
