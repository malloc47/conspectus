# ADR 0086: Payload Privacy Tenet

## Status

Accepted

## Context

Both ADR 0048 (§Lock and privacy hygiene) and `docs/design.md`
(§State And Persistence) currently phrase the payload-privacy
guardrail absolutely: "the log reader never selects
`feedback_log_body`" and, more sweepingly, "never selects
privacy-sensitive payload columns."

The absolute wording overstates what the codebase actually does.
Several sanctioned features intentionally read payload content:

- The **opencode session preview** query (ADR 0013 / ADR 0023
  lineage) selects `json_extract(data, '$.text')` from
  opencode's message table so the TUI's session row can show
  the last message. The preview is length-capped and rendered
  in the row's right-hand column.
- The **native transcript viewer** (ADR 0052, H-VIEWER-NATIVE-*)
  reads harness transcript files end-to-end and renders
  assistant, tool-use, and user turns in a full-screen modal.
  The viewer is opt-in on `v` / Enter; the surfaced content is
  exactly what the operator saw in the agent's own UI.
- The **codex state reader** (ADR 0048) reads
  `threads.first_user_message`, caps it, and normalizes it for
  the same preview surface.

These are load-bearing operator-facing features. The
"never-reads-payload" wording implies they are guardrail
violations, which is misleading — they exist because the
operator explicitly needs the content and Conspectus is the
appropriate surface for it.

At the same time, the guardrail exists for good reasons. A
significant subset of harness-DB / state-DB / log readers do
have to stay payload-free — they run implicitly, without
operator gesture, and their evidence flows into resolver
scoring and the graph rather than the visible UI. Weakening
the wording without preserving that distinction would license
unsafe reads.

The H-ADR-001 story records the drift and asks for a tenet
that matches the code. This ADR is that tenet.

## Decision

Payload access in Conspectus follows a **three-tier invariant**
graded by the reader's purpose. Every current reader belongs to
exactly one tier, and every future reader is placed in one
before landing.

### Tier 1 — Attribution and identity readers: no payload.

Readers whose evidence flows into `AgentSessionNode` identity,
`MuxSessionNode` attribution, resolver candidates, or
provenance metadata **never read payload columns**. Payload
here means user message content, assistant response content,
tool-call bodies, transcript bodies, or any harness-authored
data whose primary purpose is the model conversation.

This tier includes:

- Every reader in `discovery/harness/*` that populates
  `AgentSessionNode` fields other than
  `last_message_preview` / `title`.
- Codex state and log readers per ADR 0048, except the capped +
  normalized `threads.first_user_message` read described in
  Tier 2 below.
- Opencode state readers that populate identity / attribution.
- Every reader that produces `GraphLink` candidates.
- Every reader whose output feeds the resolver's evidence
  weights.

Attribution readers stay payload-free because their output
governs graph state, resolver decisions, and cross-session
inferences. Payload access there would be invisible to the
operator, hard to bound, and hard to reason about at scale.

### Tier 2 — Operator-facing content features: capped, normalized payload.

Readers whose sole purpose is to surface operator-requested
content **may read payload**, subject to:

- The content is displayed to the operator, not stored in the
  graph, cached beyond the current session, or reused for
  attribution.
- The content is length-capped or truncated for display before
  it reaches the operator (single-line preview: bounded chars;
  full transcript viewer: whole document but operator-initiated
  and modal).
- The content is normalized — whitespace collapsed for previews,
  no rendering of arbitrary control sequences.
- The read path is triggered by an operator gesture (opening
  the sessions view for previews; pressing `v` / Enter for the
  viewer) or by a persistent operator preference that surfaces
  the same content.

Sanctioned Tier 2 readers today:

- The **opencode preview** query
  (`json_extract(data, '$.text')` against the last user
  message), rendered as `last_message_preview` on
  `AgentSessionNode`.
- The **codex `threads.first_user_message` read** — same
  preview surface, same shape.
- The **native transcript viewer** and its per-harness parsers
  (ADR 0052) — opt-in modal, entire transcript, no persistence
  beyond the modal's lifetime.
- Any future preview-style column or transcript surface on the
  same footing.

Adding a new Tier 2 reader requires (a) a clear operator gesture
or preference gating the read, (b) an explicit cap / normalize
step, and (c) an ADR entry documenting what payload is surfaced
and why.

### Tier 3 — Sidecar and rebuildable state readers: no payload.

Hook-sidecar records (ADR 0028), snapshot state records
(ADR 0028 / ADR 0048), and any rebuildable observation format
Conspectus writes to disk **stay payload-free**. These records
persist across sessions, are shared across processes, and may
be diffed / replayed by the daemon; they carry the same
attribution and provenance data that Tier 1 does. Payload here
would extend the operator's exposure well beyond a single
render.

Tier 3 records may reference payload indirectly — a hook
sidecar can carry the harness's own session id so Conspectus can
correlate — but never the payload content itself.

### Enforcement

The tiers are enforced by review and by the ADR that introduces
each reader. There is no runtime lint or type-level barrier:
Tier 2 vs. Tier 1 is a purpose distinction, and purpose lives
in the ADR text and the reader's docstring, not in the SQL.
When a reader's purpose changes, its ADR entry updates, and
the tier follows.

If a Tier 1 reader is later found to need payload, the correct
response is either (a) split the reader — an identity path
stays Tier 1, a preview path becomes Tier 2 — or (b) record a
new ADR explaining why the shape has changed. Silently reading
payload from a Tier 1 reader is off-charter.

## Consequences

**For ADR 0048.**

The "harness-DB readers never select privacy-sensitive payload
columns" wording is not deleted but is scoped: the log reader
(`feedback_log_body`) stays Tier 3 payload-free, and the state
reader's `first_user_message` handling is explicitly Tier 2
(capped + normalized). The Codex state reader is a compliant
Tier 2 reader.

**For ADR 0013 / ADR 0023.**

The opencode preview query is now a compliant Tier 2 reader —
it was implicitly one before, but this ADR makes the placement
explicit. No behavior change.

**For ADR 0052 and H-VIEWER-NATIVE-*.**

The native transcript viewer is a Tier 2 surface. All viewer
parser work (native or external) inherits the Tier 2
requirements: cap / normalize / operator-gated / documented in
its ADR entry.

**For future harness adapters.**

New harnesses follow the same tier assignment. Discovery,
attribution, and state readers are Tier 1 / Tier 3 by default.
A preview or viewer read is Tier 2 and needs an ADR-scoped
justification. The H-EXT-006 transcript-locator work stays
Tier 2 by construction; H-EXT-002's harness registry does
not license new payload access on its own.

**For the CLAUDE.md guardrail.**

`docs/design.md` cites this ADR from the state-persistence
section instead of restating the sweeping "never selects
payload" claim. `CLAUDE.md`'s "Start read-only unless a task
explicitly calls for persistence or link CRUD" tenet is
unchanged — payload reads at any tier are still reads, and
Tier 2 does not license writes.

**For code review.**

New harness-DB / state-DB / log readers must state their tier
in the introducing ADR. Reviewers push back when a Tier 1
reader touches payload columns and when a Tier 2 reader lacks
a cap / normalize step or an operator gesture.

## Alternatives Considered

**Delete the guardrail wording entirely.** Rejected: Tier 1 and
Tier 3 are load-bearing invariants. Deleting them would license
attribution readers to poke payload for signal, which is
exactly the scale/reasoning problem the guardrail exists to
prevent.

**Restate the wording as "never persists payload."** Rejected:
that reframes the invariant around storage rather than access,
which sidesteps rather than addresses the concern. The problem
with an attribution reader reading payload isn't just that the
payload might land in the graph — it's that the reader's
evidence weights would silently become payload-sensitive, and
that opacity is what the tenet is meant to prevent.

**Fold everything into ADR 0048.** Rejected: the invariant
spans Codex (ADR 0048), opencode (ADR 0013 / ADR 0023), the
viewer (ADR 0052), hook sidecars (ADR 0028), and every future
harness. A standalone tenet ADR is the right level; ADR 0048
now cites this one.

**Make the tiers a runtime check (a `PayloadAccessLevel`
enum, an `#[allow(payload)]` attribute).** Rejected as
overengineering. The tier distinction lives in the reader's
purpose, which is best captured in ADR text and docstrings.
Runtime enforcement would either be trivially bypassed (any
Tier 1 reader could opt into Tier 2 with a one-line change) or
overly restrictive (SQL doesn't know which columns are
"payload"). Review-and-ADR is the right enforcement grain.

**A two-tier version (payload-allowed vs. payload-free).**
Rejected: it collapses Tier 2 and Tier 3 into "not attribution,"
which loses the distinction between an opt-in operator surface
and a persistent sidecar record. Three tiers matches the actual
purposes.

## Open Questions Answered

- **Which readers may access payload?** Only Tier 2 (operator-
  facing content features), with the cap / normalize /
  operator-gesture requirements.
- **Are the opencode preview and native viewer guardrail
  violations?** No. They are compliant Tier 2 readers. This
  ADR makes their placement explicit rather than implicit.
- **Do hook sidecars ever carry payload?** No. Tier 3 records
  stay payload-free.
- **Do rebuildable snapshot state records ever carry payload?**
  No. Same reason as hook sidecars — Tier 3.
- **Does this license any new writes?** No. This is an access
  tenet, not a mutation tenet. The mutation envelope (ADR
  forthcoming per H-ADR-002) governs writes.

## Open Questions Deferred

- **The exact preview-length cap.** Different harnesses set
  different caps today (opencode preview cap in
  `discovery/harness/opencode.rs`, codex
  `first_user_message` cap in `discovery/harness/codex/`).
  Harmonizing them is a UX story, not a tenet decision.
  Defer.
- **Normalization of tool-use / thinking content in previews.**
  Currently the viewer surfaces both under operator control
  (t / T keys), previews only surface the last user or
  assistant message. Whether tool-use bodies should ever reach
  a preview surface is a Tier 2 design question a future story
  can revisit.

## Related ADRs

- ADR 0013 (opencode session model) — the preview query this
  ADR classifies as Tier 2.
- ADR 0023 (session preview column) — the presentation surface
  Tier 2 reads flow into.
- ADR 0028 (hook sidecar records) — Tier 3 reference point.
- ADR 0048 (Codex state and log readers) — has the wording
  this ADR scopes; the state reader is Tier 2, the log reader
  is Tier 3, `feedback_log_body` stays never-read.
- ADR 0052 (native transcript viewer) — the largest Tier 2
  surface today.
