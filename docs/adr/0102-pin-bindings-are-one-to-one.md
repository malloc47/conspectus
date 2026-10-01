# ADR 0102: Pin Bindings Are One-To-One

## Status

Accepted. Amends ADR 0057 (binding pass, `PinStaleMux` launch) and
ADR 0084 (`pin_realized_by_session`).

## Context

The pin binding pass (ADR 0057) bound each pin on its own: it took the
active `LinkedToMux` candidates landing on the pin's mux, kept the
pin's harness, and bound the best one. Nothing stopped two pins from
binding the same session. That happens in practice. Session-file
activity evidence ties a mux to whichever transcript was written while
the mux was active, so two panes in one repo can both point at the
session that was busy at the time. One live case had `config-agent`
holding a hook `SessionStart` record for session `e44d01cb` while
`config-agent3`'s mux carried a `session_file_activity_match` for the
same session. Both pins bound it, both emitted
`pin_realized_by_session`, and the sessions view's Pins group showed
the session twice.

The general resolver already chose one mux for that session. The pin
pass ignored that choice and ranked candidates by provenance and
freshness only, so it treated the hook record and the activity
heuristic as equals.

Re-resolution made it worse. The daemon re-runs `resolve_snapshot` over
its previous resolved snapshot (hook ingest, per-class refresh), and
that snapshot still carries the pin pass's own synthesized
`LinkedToMux` links. They have `LocalPin` provenance, which outranks
all discovered evidence, so a binding confirmed itself on every later
pass. A wrong binding could never heal. The same stickiness also kept
correct bindings stable while the activity heuristic wandered, and
any fix has to keep that.

A related safety gap showed up in the same snapshot. `PinStaleMux`
means "the mux is live but no session of the pin's harness is
attributed to it", and `pin launch` reacts by `send-keys`-ing the
launch argv into the pane. A pane can run the harness without Conspectus
being able to name its session. Pane-process evidence sees the
process, but no transcript is tied to it, or the only matching session
went to another pin. Typing the launch argv there lands in a live
agent's prompt, which ADR 0028 forbids outright.

## Decision

1. **A session realizes at most one pin.** The pass ranks each pin's
   candidate sessions (best link per session), then assigns across all
   pins greedily: it repeatedly takes the strongest remaining (pin,
   free session) claim. A pin that loses a shared session falls back to
   its next free candidate, and becomes `PinStaleMux` when none is left.
2. **Rank like the resolver.** Candidates order by provenance, then
   freshness, then the resolver's session ↔ mux comparator (ADR 0006
   evidence order: hook / open-file / Codex-log above activity above
   process above cwd). A hook-reported binding beats an activity-time
   heuristic in both places.
3. **Previous bindings are the weakest evidence.** Each pass drops the
   previous pass's synthesized pin links. It re-issues each previous
   `LinkedToMux` binding as a `Cached`-provenance fallback candidate,
   which loses to any current evidence but holds a pin when its
   evidence drifts to a session another pin owns. A previous binding
   is discarded when its session is gone, or when the session has no
   activity since the mux's `created_epoch`, so a mux recreated under
   the same name doesn't inherit the old session. Previous bindings
   never count as `PinAmbiguous` competition.
4. **Explain the loss.** `Diagnostic::PinStaleMux` gains
   `claimed_elsewhere: Vec<PinSessionClaim>`, which lists sessions the
   mux matched that other pins bound, and the pins that bound them. CLI
   `pin show` and the TUI pin preview render it. `PinAmbiguous` lists
   only sessions still free for the pin, so another pin's session is
   not reported as a competitor.
5. **Never relaunch into a running harness.** `pin launch` on a
   `StaleMux` pin first checks the pane-process evidence
   (`MuxContainsProcess` → `RuntimeProcess` with the pin's
   `harness_key`). If the harness is already running there, it attaches
   without `send-keys`. TUI hints say "Enter attach" in that case.

The snapshot format version moves to 3 for the new diagnostic field.

## Consequences

- The Pins group can no longer show one session twice, and
  `pin_realized_by_session` is injective.
- Wrong bindings heal on the next pass that sees better evidence,
  including inside a long-running daemon.
- A pin whose only evidence is a heuristic now reads `stale` when a
  better-evidenced pin owns that session, where it used to read
  `bound` to the wrong session. The stale row says which pin owns the
  session. A pin's own previous binding survives that case.
- The ADR 0057 `send-keys` relaunch path now only types into panes
  without a visible harness process. A pane whose harness process
  discovery can't see (non-default sockets, process scan disabled)
  behaves as before.
- Older `graph.bin` files fail the version check and are rebuilt cold
  (ADR 0082).

## Alternatives Considered

**Dedupe in the TUI only.** Hiding the second row fixes the symptom,
but `pin list`, `pin show`, `pin_realized_by_session`, the alias overlay
and the ADR 0058 sidecar would all still record two pins for one
session.

**Bind pins from the resolved relationships instead of candidates.**
The resolver's one-mux-per-session choice already exists, but it runs
after the pin pass and takes the pin pass's synthesized links as
input. Reordering would need a second resolve pass. The assignment
here reaches the same answer with the same comparator in one pass.

**Optimal (maximum) matching.** A matching that maximizes bound pins
could give a weak-evidence pin a session that a strong-evidence pin
wanted. Greedy by evidence strength is predictable: the pin with the
best evidence keeps the session.

**Drop previous bindings without carrying them forward.** This heals
wrong bindings but makes correct ones flap whenever activity evidence
moves to a neighbouring pane's session. The dev tree hit this right
away.
