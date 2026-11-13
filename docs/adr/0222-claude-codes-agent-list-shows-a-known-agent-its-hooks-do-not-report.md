# ADR 0222: Claude Code's Agent List Shows A Known Agent Its Hooks Do Not Report

## Status

Accepted, awaiting the operator's approval (backlog CSP-871 asks for it;
until then this record and its code can be reverted together). A
further application of
[ADR 0207](0207-live-state-sources-beyond-hooks-need-their-own-decision.md)
decision 4, for a hook record that is *absent*. Amends decisions 2, 7,
and 10 of
[ADR 0218](0218-claude-codes-agent-list-repairs-a-live-state-its-hooks-left-stuck.md),
whose program, bounds, parser, whitelist, switch, and capability entry
stand unchanged. Uses the read rule of
[ADR 0203](0203-live-state-is-a-neutral-observation-with-a-closed-state-set.md)
as it is and adds a step to the binding of
[ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md).
Adds no program, subcommand, socket, signal, or write path (ADR 0117,
ADR 0143), and no snapshot field.

## Context

ADR 0218 runs `claude agents --json` only to repair a hook entry stuck in
`working` or `waiting`, and gave a session with no hook entry nothing
(decision 7): the poll ran only while some hook entry was due, so a
hook-less session's state would appear only when another session
happened to be stuck, and then hold, since `waiting` and `ready` do not
age while their process lives (ADR 0203 rule 3).

A Claude Code session whose hooks are untrusted (the workspace trust
dialog), turned off (`disableAllHooks`, `--bare`, managed hooks only), or
not installed has no live state at all, and `hook status` says
`silent`. The list Claude Code keeps of its own sessions does know their
state. Showing it needs three things ADR 0218 left open: a cadence of
its own, a binding with no hook entry to lean on, and a way for the
state to age when polls stop.

What discovery already knows: the process tree and the hook sidecar
record each running Claude Code main agent as a `RuntimeProcess` node,
with links to the session it runs where the tree could tell; `hook
status` already counts the running agents no store entry describes
(`1 running agent has no live state`).

## Decision

1. **Who.** An *unreported agent* of a harness is a main-agent
   `RuntimeProcess` of that harness in the graph whose pid runs now and
   that no store entry of the harness describes, by the recorded pid or
   by a session the graph says the process runs (the rule `hook status`
   uses for its count, now shared). Only an unreported agent can show
   state from the poll alone.

2. **Cadence and cost.** The repair poller of ADR 0218 also runs while
   its harness has at least one unreported agent, under the same bound:
   at most one attempt per 30 s, in the same processes (the daemon's
   class cycles, a daemonless TUI's background refresh worker), never in
   a one-shot command, on the TUI's UI thread, or in its one-second
   overlay. That is one call of a third to two thirds of a second every
   30 s while such an agent runs, and none while every Claude Code agent
   reports through its hooks or none runs. `CONSPECTUS_DISABLE_LIVE_POLL`
   still builds no poller.

3. **Binding.** After the repair fold, each agent the latest poll lists
   binds from the poll alone when all of these hold:
   - its pid is an unreported agent's, and the process with that pid is
     the one the poll saw (start times equal when both are known);
   - its session is found: the one session the graph says that process
     runs, else the live session node of the harness whose native id is
     the one the poll gives;
   - that session's slot holds no binding.

   It never binds a pane: the poll names none. An agent the poll lists
   without a pid, or whose process discovery does not know, binds
   nothing, so no state appears because another agent made the poller
   run.

4. **What it asserts, and how it ages.** The state and detail of ADR
   0218 decision 4, with source `harness_poll`, since the first of the
   consecutive polls that read it, observed at the poll. The observation
   records **no pid**, so a reader treats its process as unchecked
   (liveness `unknown`) and ADR 0203's rules age it with no heartbeat:
   - while the agent stays unreported and known, a poll every 30 s
     refreshes it;
   - a newer poll that no longer lists it, or a process that has ended
     (the binder checks the listed pid on every bind, so the TUI drops it
     within a second), removes it at once;
   - polls that stop (the call fails, `claude` is gone, the poller is
     gone) leave the last poll: `working` turns quiet after the
     ten-minute quiet window, and every state turns `unknown/stale`
     fifteen minutes after the last poll that listed it, as a pane
     title's state does when probes stop (ADR 0217).

5. **Hook entries win outright.** An agent with any store entry is not
   unreported; ADR 0218's repair rules apply to it, and once its hooks
   report, the entry binds and the poll's word on its own is gone.

6. **Where it shows.** Like any binding: the row cell, the header's and
   group headers' counts, `W`, the detail pane, `node show`, `table
   --columns +live`, and the graph JSON. `hook status` adds to the
   harness's health line how many agents the saved graph shows from the
   agent list (`1 agent shown from its agent list`), counting every
   `harness_poll` binding on the harness's sessions, and on panes while
   it is the only harness with a poll.

## Consequences

- A Claude Code agent with untrusted, disabled, or missing hooks shows
  `working`, `waiting`, or `ready` within about 30 s with `conspectus
  serve` or a daemonless TUI running, when discovery knows its process
  (the process tree finds agents in the active panes of tmux sessions).
  One outside tmux, or in a pane the tree does not reach, still shows
  nothing, and `hook status` still says `silent` for it.
- An operator who never installs the hooks pays one call every 30 s
  while Claude Code runs.
- The poll's `ready` cannot tell a finished turn from an interrupted one,
  and its `waiting` cannot say which question; the hooks can.
- The poll-only state of an agent whose polls keep failing reads
  `unknown` after fifteen minutes instead of holding.
- The output shape and the pid match are still unverified against a
  recording (backlog CSP-822); a recording changes the parser's table and
  nothing else.

## Alternatives Considered

- **Keep ADR 0218 decision 7** (no state without a hook entry). Leaves
  hook-less sessions dark though the harness reports their state.
  Rejected, subject to the operator's approval.
- **Bind every listed session the store has no entry for, by session
  id.** States would appear for sessions no process is known for, only
  while some other agent keeps the poller running: the accident decision
  7 named. Rejected for the known-process rule.
- **Record the listed pid** so the binding's liveness is checked. Then
  `ready` and `waiting` never age while the process lives, so a poll-only
  state would hold after polls stop. Rejected; the pid is checked at bind
  time to drop an ended agent, and left off the observation.
- **A heartbeat of the poll interval.** It would only shorten the quiet
  window for `working`; `ready` and `waiting` still would not age.
  Rejected for the unchecked-process rule.
- **Poll whenever any Claude Code process runs.** Costs a call every 30 s
  for agents whose hooks already report. Rejected.

## Open Questions Answered

- *Does a poll ever create a node?* No. It binds only to a live session
  node discovery already made.
- *Does it widen what Conspectus reads?* No: the same call, parser, and
  whitelist as ADR 0218, more often while an unreported agent runs.
