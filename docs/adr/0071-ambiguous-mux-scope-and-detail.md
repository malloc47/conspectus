# ADR 0071: Ambiguous Mux Surface Moves From Session Row To Group Detail

## Status

Accepted

## Context

Today an agent session with ≥2 active `LinkedToMux` candidates
becomes expandable in the Sessions view; its candidates render as
`AgentSessionMuxCandidate` child rows beneath the session, marked
with the resolver-preferred entry highlighted (see ADR 0024 and
the docstring at the top of `src/tui/rows/sessions.rs`).

The original intent was "show the operator what the candidates
are so they can decide." In practice, when multiple sessions
share the *same* ambiguous mux set (the showcase scenario
reproduces this with two claude-code sessions both attributed to
`project` and `ambiguous`), the candidate rows render once per
session — so the same two muxes appear duplicated under each
session, multiplying line count by the number of ambiguous
sessions without adding information.

The operator's actual decision is not "which session goes with
which mux"; it's "which mux am I attaching to" or "which session
do I bind to this mux." The information naturally lives at the
shared parent (workspace, repo, or checkout) — that's where the
ambiguity is, not duplicated under each child session.

## Decision

1. **Drop `AgentSessionMuxCandidate` rows from the Sessions
   view.** Session rows keep the `MuxIndicator::Ambiguous` chip
   (`◐`) so the eye still sees the flag, but the expandable
   candidate subtree is gone. Sessions with ambiguous mux state
   render as leaves (still expandable when lineage children
   exist).
2. **Surface ambiguous muxes on the shared-ancestor group's
   detail pane.** For each `Workspace`, `Repo`, and `Checkout`
   node, the detail builder emits an `Ambiguous muxes` section
   listing every mux that:
   - has ≥2 active `LinkedToMux` candidate links from agent
     sessions whose **closest common ancestor** is this group
     node, **and**
   - the resolver has not selected a winner for.

   "Closest common ancestor" walks workspace → repo → checkout.
   If two ambiguous sessions share only a repo, the section
   lives on the repo. If they share a workspace, on the
   workspace. Single-session ambiguity attaches to that
   session's natural parent (its associated workspace if
   A-class, else its repo).
3. **`a` on a group row defaults to the single ambiguous mux.**
   When the selected row is a group node and its
   `Ambiguous muxes` section contains exactly one entry, the
   `a` accelerator (`Action::Attach`) attaches to that mux —
   same code path as `a` on a `RowKind::AgentSession` with a
   resolved `Attached` mux. When the section is empty or has
   multiple entries, `a` falls through to a status hint
   ("3 ambiguous muxes — pick one in detail or `b` to bind")
   matching today's `a`-on-an-unattachable-row behavior.

## Consequences

- **Sessions view shrinks.** In the showcase, the two
  ambiguously-attributed claude-code sessions stop rendering
  four redundant `tmux:project` / `tmux:ambiguous` child rows
  and read as two leaves instead.
- **The operator's eye lands on the shared parent.** The
  ambiguity is no longer a per-session decoration; it shows up
  once, where the muxes actually live, so the decision is
  visible at the right scope.
- **`a` becomes useful on group rows.** A workspace or repo row
  with exactly one ambiguous mux now has a sensible default
  action: attach to that mux. The most common case (one
  ambiguous mux per group) gets a one-key path.
- **Multi-mux groups stay safe.** `a` on a group with multiple
  ambiguous muxes does not silently pick one; it points the
  operator at the detail section. No surprise attach.
- **`MuxIndicator::Ambiguous { candidate_count }` stays on the
  session row.** The chip is still the per-session flag; the
  detail section is the per-group catalog. Both surfaces stay
  useful.
- **`RowKind::AgentSessionMuxCandidate` becomes unreachable.**
  The enum variant stays for now to avoid touching every
  match arm; a follow-up can drop it cleanly once the row
  builders settle.

## Alternatives Considered

### A. Collapse-by-default but keep the candidate rows

Hide the candidate rows until the operator expands a session.
Simpler change; preserves the per-session candidate view for
operators who want it. Rejected because the operator hadn't
been opening the subtree (the screenshot that triggered this
ADR showed the rows fully expanded by default in the showcase
fixture), and the duplication noise was a recurring complaint
even when collapsed — the `▶` glyph still multiplies across
ambiguous sessions.

### B. Move the candidates to the per-session detail pane

Keep ambiguity per-session by listing the candidates on the
session's own detail. Rejected because the actual operator
decision spans sessions ("which mux am I attaching to" answers
itself once, not per child), so the per-session view splits
information that wants to stay together.

### C. Pick the freshest mux on `a` when multiple ambiguous

`a` auto-attaches to the most recently active ambiguous mux.
Rejected because it silently picks; a wrong guess is harder to
diagnose than a no-op status hint.

## Open Questions

- Should the `Ambiguous muxes` section also include a count of
  the candidate sessions per mux (e.g. "2 sessions")? Probably
  yes; pin in implementation if it makes the rendered line
  long.
- Should `b` (bind) on a group row open the pin-bind picker
  pre-scoped to the group's sessions? Out of scope for V1; the
  per-session `b` path already covers it once selection moves
  to the conflicting session.
- Should the empty `Ambiguous muxes` section be suppressed
  entirely or render with a placeholder line? Suppress;
  matches every other relation section's empty-elision rule.
