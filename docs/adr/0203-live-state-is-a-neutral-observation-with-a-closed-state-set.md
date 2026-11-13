# ADR 0203: Live State Is A Neutral Observation With A Closed State Set

## Status

Accepted. Part of the live agent state plan (`docs/plans/live-state/`)
with ADRs 0204, 0205, 0206, and 0207. Extends the data model; adds no
write path (ADR 0143) and relies on ADR 0086 (payload privacy) and ADR
0140 (absence is not evidence of absence).

## Context

The graph says which sessions and muxes exist and how they relate. It
does not say what a session is doing now: whether an agent is waiting
for the operator, finished and ready for a prompt, or still working.
Other agent orchestrators lead with that (their rows carry a state mark,
their headers count what needs the operator), and it is the most
common reason to open Conspectus at all.

Each harness reports state in its own words: `PermissionRequest`,
`session.status busy`, `ui_prompt_start`, `agent-turn-complete`. They
disagree on what a "stop" means (a finished turn, an interrupt, a
failure), whether a subagent keeps running after the turn ends, and
whether there is a signal at all. The research
(`docs/plans/live-state/research/`) found that every harness exposes
enough through hooks or plugins to place a session in one of eight
states, and that the gaps (Esc interrupts, approvals granted, killed
processes) are the same kind in every harness.

The graph is sparse and provider-neutral (design requirements). A
session with no hook installed has no evidence, which is not the same as
being idle.

## Decision

1. **The state set is closed and harness-neutral.** `LiveState` is one
   of `working`, `waiting`, `ready`, `compacting`, `error`, `starting`,
   `exited`, `unknown`. Each state has a closed set of details
   (`LiveDetail`):

   | State | Details |
   |---|---|
   | `working` | `generating`, `tool`, `retrying`, `rate_limited` |
   | `waiting` | `permission`, `question`, `plan`, `input` |
   | `ready` | `finished`, `interrupted` |
   | `compacting` | none |
   | `error` | `rate_limited`, `auth`, `overloaded`, `failed` |
   | `starting` | none |
   | `exited` | `ended`, `killed` |
   | `unknown` | `stale` |

   Harness words (`PermissionRequest`, `session.idle`) are mapped to
   these in the adapters and plugins. The model never sees them. A
   harness signal that fits no state is dropped, not stretched.

2. **A `LiveObservation` is the stored fact.** It holds the state and
   detail, `subagents` (count of active child agents), `via_subagent`
   (the state is raised from a child to the root session), `since_ms`
   (when this state began), `observed_ms` (the latest evidence),
   `source` (`hook` now; room for others), the process `pid` and its
   start time, and `heartbeat_s` (0 when the emitter sends none). It
   holds enums, counts, ids, and times, never text (ADR 0086 Tier 3).

3. **Effective state is computed at read time, not stored.** From the
   observation, `now`, and a liveness answer (`Alive`, `Dead`,
   `Unknown`):

   1. `Dead`, or a changed start time: `exited/killed`.
   2. `working`, `compacting`, and `starting` older than the quiet window
      (10 minutes for event-only sources, three heartbeats for sources
      that send them) with a live process, or one that cannot be checked
      until rule 4 applies, remain that state and are marked **quiet**. A
      long build is still work. They never decay to `ready`.
   3. `waiting`, `ready`, and `error` do not age while the process
      lives. An operator can leave a prompt for hours.
   4. With liveness `Unknown`, past 15 minutes every state except
      `exited` becomes `unknown/stale`.
   5. `starting` older than two minutes becomes `unknown/stale`.

   "Older" is the age of the latest evidence (`observed_ms`), not of the
   state (`since_ms`), so a heartbeat or a repeated event keeps a state
   fresh; a state passes a window only when strictly older than it. A
   recorded `exited` is never changed by time or liveness.

   The result is `(state, detail, confidence)`, with confidence `fresh`,
   `quiet`, or `stale`.

4. **No entry means no evidence.** A session without a hook, or whose
   entry was dropped, has no live state and is shown as such. It is
   never shown as `ready`.

5. **A mux has no state of its own.** It has counts over the sessions it
   hosts and any pane-only entries (`LiveCounts`: one counter per
   state). Which state a mux row shows is a display decision. The model
   offers the counts and a default precedence function (`waiting`, then
   `error`, then `ready`, then `working`, then the rest), which a viewer
   may replace.

6. **"Seen" is not part of the observation.** Whether the operator has
   looked at a finished turn is the viewer's concern, derived from
   `since_ms` and the mux's last attach time, or held in viewer memory.
   The observation keeps `since_ms` accurate so either is possible.

7. **The vocabulary lives in `src/model/live.rs`** (pure, serde and
   rkyv). Adapters and the store depend on it; it depends on neither.

## Consequences

- Every surface (CLI, JSON, TUI, library) shares one vocabulary and one
  staleness rule, so a state cannot mean different things in different
  views.
- The quiet flag avoids both failure modes seen in other tools: marking a
  long job idle, and showing a dead agent as working forever.
- New harnesses add a mapping, not a state. A state a harness cannot
  express (Pi has no built-in permission prompt) is simply never emitted.
- `compacting` and `rate_limited` are included because the harnesses
  report them cheaply. No other orchestrator surveyed shows compacting,
  so a viewer may fold it into `working`.
- A second library-API surface (`model::live`) joins the stable `model`
  module (ADR 0015).

## Alternatives Considered

- **Three states only (waiting, ready, working).** Matches the operator's
  question but throws away `error`, `exited`, and `unknown`, which are
  exactly the cases where the other three would lie. Rejected.
- **Free-form state strings per harness.** Fastest to build; makes every
  consumer re-learn each harness. Rejected (provider-neutral core).
- **Storing the effective state.** Needs a writer that wakes up when time
  passes, so the daemon would write on a timer. Rejected; the effective
  state is a pure function of the observation and `now`.
- **Storing "seen".** A persisted flag is a write the operator did not
  ask for (ADR 0143) and a source of flicker in tools that tried it.
  Deferred to the viewer.

## Open Questions Answered

- *Is a stale `waiting` shown as waiting?* Yes, while the process lives.
  The cost of a false "still waiting" is a glance; the cost of dropping a
  real one is a stuck agent nobody noticed.
- *Do subagents get their own rows?* Not in the model. Child agents
  contribute to `subagents` and, when blocked, raise `waiting` with
  `via_subagent` to the root session.
