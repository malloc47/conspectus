# ADR 0254: The Launch Form Starts As A Copy Of The Selected Entity

## Status

Accepted. Amends [ADR 0096](0096-ephemeral-harness-mux-launch.md) (the TUI
surface): the form is seeded from the entity the operator is looking at
beyond its name. The name's seed, which follows the series of the mux the
operator is looking at, is unchanged. No new key, setting, or write path.

## Context

`N`, or `m` then **Launch harness in new mux…**, opens the launch form
seeded from the selected row, but only in part. The name continues the row's
series (`worker-1` gives `worker-2`) and the cwd is the row's when it is a
live mux, session, checkout, or repo. The harness was always the first
registered one, the argv was empty, and the pin choice was always `don't
pin`, whatever the row was. Launching another mux like the one under the
cursor meant retyping its harness, its flags (skip permissions and the
like), and whether it is pinned. The cwd was also taken without checking that
the directory still exists, which a stopped mux, a removed worktree, or a pin
on a deleted checkout can fail. The operator (2026-11-24) asked for the form
to start as a copy of the row, as the name already does.

## Decision

1. **One entity seeds every field.** The entity is the one the name seed
   follows: the right pane's while it has focus, else the selected row's.
   One entity seeds the same form in every view that shows it.
2. **The harness is the entity's, when the form can pick it.** A mux's is
   the harness of the command it runs (a stopped mux's, of its record); a
   session's is its own; a pin's is its effective one. A harness the form does
   not list is not used, and the form keeps its first.
3. **The arguments are how the entity was launched.** A mux or a pin
   gives its argv; a session gives the argv of the running mux that hosts it
   when that mux runs the same harness. The form then shows the launch
   options as that argv has them, so skip permissions starts checked when the
   row has it. An argv is used only with its harness, and an argv that is just
   the harness's own default is no statement about the entity, so the form
   keeps deciding its default, which can be a launch profile's.
4. **The directory is the entity's, if it exists.** The row's current
   directory first, then its recipe's, and the first that is a directory
   now. When none is, the form seeds the directory it does with no row. This
   is the launch form's alone: the new tmux session form keeps its seed.
5. **The pin choice follows the entity's pin.** No pin: `don't pin`. A pin
   whose target is running: `launch, then pin`. A pin whose target is not
   running (stopped, planned, unknown, or missing): `pin, don't launch`. Only
   a mux or a session is read this way: a pinned PR, checkout, or repo says
   nothing about whether a new mux should be pinned.
6. **An entity that says nothing leaves the form as it was.** A group
   header, a PR, or a row with no harness, directory, or pin opens the form
   with the harness, argv, directory, and pin choice it had before.

## Consequences

- Launching another mux like the one in front of you is `N` then `Enter`,
  with the flags and the pin as the row has them. Everything stays editable,
  and the harness change still carries the argv across as before.
- A stopped mux seeds from its history record, so `N` on a ghost row starts a
  fresh mux like the stopped one, in its directory when that is still there.
- A running mux's launch argv is what its pane was started with, which is
  tmux's rendering of it; a pin's argv is the exact tokens it was launched
  with and wins over it.
- A mux inside a launch wrapper (`nix develop -c claude`) seeds its wrapper
  command as its argv, as it ran, and the harness is the one the command names.
- The session row's pin state is the session's own: a session in a pinned mux
  opens the form unpinned unless the session itself is pinned.

## Alternatives Considered

- **Seed only the harness.** It is the largest retyping, but a launch from a
  row that skips permissions would silently drop the flag.
- **Try every later directory when the first is gone.** It gives a better
  start sometimes and a surprising one other times. The row's current
  directory and its recipe's are the two the entity itself names.
- **Pin whenever any kind of pin is on the row.** A pinned PR or checkout is
  not a launch of anything, and a new mux pinned because of it would be a
  guess.
- **Read the pin's overrides, not its effective fields.** The effective fields
  are what a rebuild of the pin would launch, which is what the operator is
  copying, and they already layer the overrides over what is live.

## Open Questions Answered

- **Which row counts as pinned?** The one the Pinned view or the row's pin
  marker shows: a pin whose target is the entity. A stopped mux with a pin on
  its history record counts.
- **What if the harness in the row is not installed?** The form lists the
  registered harnesses, and a harness outside the list is not seeded.
