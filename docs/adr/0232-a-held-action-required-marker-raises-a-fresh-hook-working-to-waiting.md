# ADR 0232: A Held Action Required Marker Raises A Fresh Hook `working` To `waiting`

## Status

Accepted. Carries out the operator's decision of 2026-11-13 on the
follow-up left open in the consequences of
[ADR 0217](0217-a-codex-panes-title-fills-in-its-live-state-where-hooks-cannot.md):
a second carve-out to the rule of
[ADR 0207](0207-live-state-sources-beyond-hooks-need-their-own-decision.md)
decision 4, that a fresh hook record wins over a poll, beside the
answered approval of ADR 0217 decision 7 rule 4. Amends ADR 0217
decision 7 (a fifth rule, and rule 2's "apart from rule 4") and its
third consequence. Reads the same marks, from the same pane listing
([ADR 0221](0221-every-panes-title-is-read-from-the-pane-listing.md)),
under the same tier and switch as ADR 0217 (ADR 0086 Tier 2,
`CONSPECTUS_DISABLE_PANE_TITLES`). Adds no program, subcommand, socket,
signal, write path, or tmux call (ADR 0117, ADR 0143), and changes no
snapshot format.

## Context

With Codex hooks installed, a wait no hook reports (a question, an MCP
elicitation, a sign-in prompt) reads `working` from the last tool or
prompt event until that entry goes quiet ten minutes later, although the
pane's title already says `[ ! ] Action Required`. ADR 0217 lets the
title fill in only where hook evidence is absent, quiet, or stale, plus
one carve-out in the other direction (an answered approval turns a
fresh `waiting` into `working`). The operator gets no `!` for those
waits, in the header count, in `W`, or on the mux's mark, for as long as
ten minutes, when it is the very state those marks exist for.

The marker is the strongest signal Codex gives: it is up exactly while
the bottom pane needs the operator, and no hook can contradict it, since
a hook that fires while it is up (the tool that was waiting ends, a
later prompt) also changes the entry's latest evidence.

## Decision

1. **A fresh hook `working` turns `waiting`.** For a hook entry whose
   effective state is `working` with fresh confidence, when the pane's
   title has read `waiting` (either form of the `Action Required` marker)
   on two consecutive probes, both after the hook entry's latest
   evidence, the binding shows `waiting`, detail `input`, source
   `pane_title`, in its place. It starts when the first probe of that run
   after the entry's evidence ran (the run's own start when it began
   after the evidence, else the earlier of the last two probes, as for
   the answered approval). It keeps the hook entry's process, its
   liveness, and its subagent count. The store's entry is not rewritten.

2. **Two probes, both after the hook event.** One probe may be a stray
   frame, or may have landed between the hook's write and Codex's redraw;
   a probe from before the hook's latest event says nothing about the
   agent that went on after it. A probe at the very moment of the event
   is not after it. The next hook event (the tool ending, the turn
   ending) is newer than every probe that read the marker, so the hook
   entry takes over again until two later probes read it.

3. **Only `working`.** A hook entry's `ready`, `error`, `compacting`,
   `starting`, and `exited` are never changed by a title, as before, and
   a `working` whose process was found gone reads `exited` and stays
   there. A `working` that has gone quiet, and a stale or `unknown`
   entry, already give way to a `waiting` mark (ADR 0217 decision 7 rule
   3); this adds the fresh case. A `waiting` entry stays `waiting`, with
   no change to its answered-approval rule. A title that says `working`
   or nothing changes a fresh `working` as little as before.

4. **Cadence and latency.** As for the answered approval: the source
   rides the mux probe, so a wait reads `waiting` about two rounds (about
   ten seconds with `conspectus serve`, one refresh pair in a daemonless
   TUI) after the marker first shows, and a one-shot command, which has
   one probe, never raises. The TUI's one-second overlay binds again
   against the marks its snapshot holds and never asks tmux.

## Consequences

- A Codex that asks a question, an MCP elicitation, or a sign-in shows
  `!` within about ten seconds, in the header count and `W`, instead of
  after ten minutes of silence. It reads `working` again once the title's
  marker is gone and a later hook event takes over, within about a
  round.
- A thread whose name starts with the marker reads as waiting while
  its hooks say it works, for two probes at least: the case ADR 0217
  already names for an idle one. The title cannot tell them apart.
- A Codex with its title animations off shows the marker the same way,
  and the carve-out holds. With a custom `tui.terminal_title` that drops
  the activity item, nothing changes: the source is silent.
- The detail pane and `node show` say `source: pane_title` for the
  raised state, and the hook entry keeps its own record in the store.

## Alternatives Considered

- **One probe.** Raises sooner, at the price of showing a wait for a
  frame that was a stray title.
- **Raise after a fixed time past the hook's write.** A clock in the
  binding, which takes none; the probes' own times serve.
- **Raise only a quiet `working`.** Already the rule; it waits ten
  minutes, which is the gap this closes.
- **Keep the hook's word.** The decision of ADR 0207 decision 4 as it
  stood; the operator chose the exception.

## Open Questions Answered

- *Does it apply to Claude Code or another harness?* No: only Codex's
  titles are read (ADR 0217 decision 2).
- *Does it reach a launcher's `node` pane?* Only where Codex's hook
  entry names the pane, as for every title read there (ADR 0221).
