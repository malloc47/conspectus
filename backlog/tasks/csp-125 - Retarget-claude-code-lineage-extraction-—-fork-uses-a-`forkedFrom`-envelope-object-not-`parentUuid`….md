---
id: CSP-125
title: >-
  Retarget claude-code lineage extraction — fork uses a `forkedFrom` envelope
  object, not `parentUuid`…
status: Done
assignee: []
created_date: '2026-05-17 22:08'
labels:
  - h-lineage
milestone: m-11
dependencies: []
ordinal: 185000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Retarget claude-code lineage extraction — fork uses a `forkedFrom` envelope object, not `parentUuid`; `/compact` is in-place.

- Resolution (1, 5): the claude-code adapter now reads `forkedFrom`
  from the first uuid-bearing record. When `forkedFrom.sessionId`
  matches another discovered session in the same project directory
  the link resolves to a concrete `AgentSession` target with
  `lineage_kind = "fork"`; otherwise it is preserved as
  `UnresolvedEndpoint` evidence keyed by parent session id.
  `forked_from_message_uuid` is carried in source metadata for
  future point-in-time use. `forkedFrom` wins over `parentUuid`
  when both are present. Regression tests cover resolved fork,
  unresolved fork, bare fork (no envelope, no lineage), and
  forkedFrom-vs-parentUuid precedence. Validated on live state
  2026-05-17: `332aa87b-…` (fork-with-history of `81f4a0ef-…`)
  renders `LINEAGE = …50b40572`; `926c6991-…` (bare fork) shows
  `—`.
- Resolution (2): the bare-fork variant stays as `—`. The
  transcript carries no on-disk signal, and side-channel inference
  (IDE state files, fork-time proximity, sibling session listings)
  is high-effort for a UI gesture that may not even be reachable
  from current claude-code releases. Revisit only if the gesture
  becomes common or claude-code publishes a structural pointer.
- Resolution (3): in-place `/compact` is **not** modeled. ADR 0018
  keeps `AgentSession` at session-file granularity, so a within-
  session `type: "summary"` record cannot be a `parent_session`
  edge — both endpoints would resolve to the same node. A
  regression test (`in_place_compaction_summary_record_emits_no_lineage`)
  locks this in: a transcript with a mid-stream summary record
  produces one `AgentSession` and zero lineage candidates. The
  adapter's module-level doc comment documents the policy.
- Resolution (4): no capability constant changes. The standalone
  claude-code adapter does not emit a `lineage_fidelity` field —
  that lives in atelier's per-fork TOML and is interpreted by the
  delegation flow. The fork lineage emitted here is already
  `Provenance::StrongDiscovered` / `Confidence::High`, which is
  the strongest tier available. If a future atelier fork record
  advertises claude-code as Native for fork lineage, no Conspectus
  code change is needed.
- Context: CSP-118 assumed compaction (or a similar successor
  operation) produces a new session jsonl whose first uuid-bearing
  record's `parentUuid` points at the predecessor's leaf uuid.
  Manual validation against `~/.claude/projects/` on 2026-05-17
  (claude-code 2.1.129) found that assumption wrong:
  - `/compact` is in-place: invoked mid-session, it keeps appending
    to the same session jsonl rather than creating a successor file.
    No cross-session `parentUuid` is produced.
  - IDE session fork (fork-with-history variant, exercised by
    forking `81f4a0ef-…` into `332aa87b-…`) **does** record
    structural lineage, but on a different field than the adapter
    looks at. The child file copies the parent's records and tags
    each copied record with an envelope field
    `forkedFrom = {sessionId: <parent session id>, messageUuid:
    <original uuid in parent>}`. Of 961 records in the
    `332aa87b-…` file, 510 carry `forkedFrom` (the copied parent
    prefix) and 451 do not (the fork's own new records). The
    child's own `parentUuid` chain still starts at `null` for the
    first record, so the adapter's current first-record-parentUuid
    heuristic misses this entirely.
  - A second fork (`926c6991-…`) carried zero `forkedFrom`
    records (15 records, 0 tagged). Likely a different IDE
    affordance ("fresh session from here" vs "fork with history")
    or an older code path — investigate which UI gesture produces
    which shape.
- Scope: (1) Extend the claude-code adapter to read `forkedFrom`
  from the first uuid-bearing record (or the first tagged record
  if the envelope ordering differs). When present, emit a
  `parent_session` candidate with `lineage_kind = "fork"`,
  fidelity `native`, source = child session, target = the session
  identified by `forkedFrom.sessionId`. Preserve the messageUuid
  in source metadata for future point-in-time lineage. (2) Decide
  how to handle the "no-`forkedFrom`" fork variant exemplified by
  `926c6991-…` — likely treat as unresolvable from transcript
  alone and rely on future side-channel inference. (3) If in-place
  compaction is the durable behavior, design within-session
  lineage: parse `type: "summary"` records and treat the
  pre-summary leaf uuid and post-summary first user uuid as an
  intra-session compaction boundary; pick whether to model the
  pre-compaction span as a distinct logical session or as an
  annotation on the same `AgentSession`. (4) Once `forkedFrom`
  extraction lands, upgrade claude-code's lineage capability from
  `Approximate` to `Native` for the fork-with-history case;
  `/compact` and the bare-fork variant remain `Unsupported` until
  addressed. (5) Add regression tests covering: a fork transcript
  with a `forkedFrom` envelope (resolved-parent case), the same
  pointing at a missing parent (`UnresolvedEndpoint`), and a fork
  transcript with no `forkedFrom` records (no lineage emitted).
- Tests: a fixture transcript containing a `type: "summary"` record
  surrounded by user/assistant records exercises the in-place case;
  keep the existing
  `lineage_pointer_is_read_from_first_uuid_bearing_record_not_envelope`
  regression test for the cross-session path so we do not regress if
  a future claude-code release reintroduces successor files.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-LINEAGE-006`
