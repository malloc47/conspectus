---
id: CSP-527
title: 'Create bare tmux sessions from within Conspectus (no pin, no agent)'
status: Done
assignee: []
created_date: '2026-08-08 03:23'
labels:
  - h-mux-new
milestone: m-18
dependencies: []
ordinal: 553000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: operators currently drop out of Conspectus to run a
  plain `tmux new-session -s <name> -c <cwd>` when they want a bare
  console session rooted in a repo/checkout. The mux view should
  support this in-place, distinct from the pin-launch path (which
  always creates a pin + attributes a harness).
- Envelope: sanctioned by ADR 0087 category 3 (operator-initiated mux
  lifecycle: rename / new-session / attach). No new mutation category
  needed. `MuxBackend::new_session` primitive already exists
  (`src/discovery/tmux/mod.rs:112`) and is used by pin launch; this
  story adds a no-argv, no-pin caller.
- Scope:
  - Design note (short — in `docs/design.md` under a Mux Lifecycle
    section, or extend the existing pins/worktree lifecycle prose)
    distinguishing "bare mux" (no pin, no agent) from "pin launch"
    and "worktree stream". Consider whether a full ADR is warranted;
    likely a paragraph is enough if no new envelope categories are
    needed.
  - TUI: menu-first "New tmux session…" action in the mux-context
    action menu, plus a lowercase `n` hot key (uppercase `N` is the
    worktree-backed pin create shortcut per ADR 0094). Prompt is a
    single-field name input; on submit, invoke `new_session` with
    no argv and hand off to the existing attach path.
  - Default cwd seeding: selected row's cwd when a Repo / Checkout /
    Mux / Session row is selected, else `$HOME`. Editable in the
    form.
  - CLI parity: `conspectus mux new <name> [--cwd <path>]
    [--socket <name>]`.
  - Backend availability: requires the tmux mutation backend already
    required by pin launch. No new capability flag needed.
  - Post-create UX: after `new_session` returns `Created`, follow the
    existing pin-launch exec-replace attach path so the operator
    lands inside the fresh session. On `NameTaken`, toast + keep the
    form open (parallel to pin create's duplicate handling).
- Non-goals: this story does not persist a pin, run an agent, tie
  the session to a worktree, or affect discovery. A bare mux is
  picked up by the existing tmux discovery on the next refresh.
- Tests: `new_session` call assertion via `FakeTmux`; snapshot the
  new form's initial state; cwd-defaulting unit tests for each row
  kind; CLI arg parse + dispatch test.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
**landed as ADR 0095**. Design memorialized in ADR 0095
(bare mux as ADR 0087 category-3, distinct from pin-launch /
worktree-stream) plus a `## Mux Lifecycle` section in
`docs/design.md`. CLI: `conspectus mux new <name> [--cwd <path>]
[--socket <name>] [--no-attach]` in `src/cli/mux.rs`, reusing the
pin-launch attach + report helpers. TUI: lowercase `n` opens a
two-field `NewMuxFormState` overlay (name + cwd; cwd seeded from
the selected row's cwd else `$HOME`; `Tab` cycles focus; `Enter`
advances/commits). On commit the TUI re-execs into
`conspectus mux new … --no-attach`, refreshes discovery, then
attaches. A dedicated `m`-keyed mux action menu (which would fold
in attach `a` / `Enter` and rename `R` for discovery alongside New)
is deferred until a second bare-mux-shape verb lands; today attach
and rename are polymorphic-across-node-kinds global bindings rather
than mux-specific menu entries. See ADR 0095 follow-ups.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUX-NEW-001`
