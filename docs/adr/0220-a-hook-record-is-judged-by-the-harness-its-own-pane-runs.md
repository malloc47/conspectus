# ADR 0220: A Hook Record Is Judged By The Harness Its Own Pane Runs

## Status

Accepted. Amends the attribution gates of
[ADR 0028](0028-hook-sidecar-mux-attribution.md) (its 2026-11-12
amendment) and of [ADR 0146](0146-the-hook-spool-is-the-durable-hook-record.md)
(amendment item 5): the gate that ignores a record when "the mux's
active pane runs another harness" now reads the record's own pane.
Adds a field to the mux probe of
[ADR 0140](0140-history-and-presence.md) and bumps the snapshot
format to 15 (ADR 0083). Adds one call of an already sanctioned tmux
read per probed server; adds no program, subcommand, socket, signal, or
write path (ADR 0117, ADR 0143).

## Context

The hook spool keeps one record per pane (ADR 0146's amendment), so two
agents in split panes of one tmux session both keep their records.
Attribution then ignores a record "when the mux's active pane runs
another harness", a gate added so a leftover Claude Code record cannot
claim a pane that now runs Codex. It read
`MuxSessionNode.active_pane_command`, because the mux node describes
only the session's active pane and the graph has no pane entity.

With Codex in one pane and Claude Code in the pane beside it, whichever
pane is not active was gated against the other pane's program: its
record was marked Ignored ("pane now running `codex`") and only one of
the two agents was attributed. Switching panes swapped which one.

The mux probe runs `tmux list-sessions -F <format>` once per server
each mux cycle (5 s with a daemon, 30 s in a TUI without one, ADR
0140), and that call answers for each session's active pane only.

## Decision

1. **The probe lists every pane.** After a session listing that
   succeeded and listed at least one session, the probe runs
   `tmux list-panes -a -F '#{pane_id}\t#{session_name}\t#{pane_current_command}'`
   against the same server and records, per pane id, the pane's session
   and the program in its foreground (`MuxProbe.panes`). One more call
   per listed server per mux cycle, a few milliseconds each; a server
   with no session, or that did not answer, is not asked. A listing that
   fails, or rows that are not pane rows (an id other than `%` and
   digits, no session), record nothing for that row or that probe. The
   panes ride in `graph.bin` with the probe and stay out of JSON, as the
   probes do.

2. **The gate reads the record's own pane.** When the record names a
   tmux pane and the probe of its mux's server listed panes, the
   harness the record is checked against is the one that pane's program
   names, by the exact process names the gate already used. A pane the
   listing does not show in the mux's session (one opened after the
   probe, or moved to another session) runs nothing known, and gates
   nothing. A record that names no pane, or a probe with no pane
   listing (an older snapshot, a failed listing, a backend without
   panes), is checked against the active pane as before.

3. **What the pane listing is not.** It is no pane entity in the graph,
   no attribution evidence of its own, and no title: the program name is
   the same field `active_pane_command` already carries for the active
   pane. The other gates (process gone, mux created after the record)
   and the per-pane winner are unchanged.

4. **Capabilities.** `list-panes ...` is already a sanctioned tmux read
   (ADR 0149's pane search). The manifest's tmux entry cites this ADR;
   no entry is added.

## Consequences

- Two agents of different harnesses in split panes of one tmux session
  are both attributed, whichever pane is active.
- A record whose own pane now runs another harness is still ignored,
  whether or not that pane is active.
- A record from a pane opened after the last probe is not gated by its
  program until the next probe lists it; the process gate still applies.
- Each mux cycle starts one more tmux client per server that has
  sessions.
- `graph.bin` moves to format version 15; older files cold-rebuild.

## Alternatives Considered

- **Drop the gate for a record from a non-active pane.** Fixes the split
  pane, but a leftover record whose pane now runs another harness would
  be judged by nothing but its process. Rejected.
- **A `display-message -t <pane>` per record.** One process per record
  per cycle, from the attribution pass, which starts none today.
  Rejected.
- **Pane nodes in the graph.** A model change for one gate, with
  identity and history questions of its own. Rejected for now.
- **Every pane's program on the mux node.** The node is identity; a list
  that changes as panes open and close would churn it and the
  process-tree fingerprint (ADR 0217's alternative). Rejected for the
  probe.
- **Replace the session listing with `list-panes -a`.** One call instead
  of two, but every session field would be read from its active pane's
  row and every recorded listing in the tests would change. Not worth it
  for the saving of one cheap call.

## Open Questions Answered

- *Does the listing read a pane's title or content?* No: its id, its
  session, and its program name.
