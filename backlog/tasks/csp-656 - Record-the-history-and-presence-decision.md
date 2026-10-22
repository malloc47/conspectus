---
id: CSP-656
title: Record the history and presence decision
status: To Do
assignee: []
created_date: '2026-10-22 01:10'
labels:
  - history-pins
milestone: m-23
dependencies: []
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0057-session-pins.md
  - docs/adr/0058-pin-session-continuity.md
  - docs/adr/0102-pin-bindings-are-one-to-one.md
  - docs/adr/0086-payload-privacy-tenet.md
priority: high
type: docs
ordinal: 714000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Session pins (ADR 0057) are today the only way to relaunch a mux after it dies with its name, launch argv, and last session, so the operator pins nearly every mux and "pinned" stops meaning anything. The plan in `docs/plans/history-rebuild-and-pin-slots.md` (reviewed 2026-10-19 and 2026-10-21) splits that apart: Conspectus records everything rebuildable it observes in a history layer, independent of pins. This ADR records the history half so the store, recording, and projection tickets build against a settled model.

Operator decision to record: launch argv is stored verbatim, including the `pane_start_command` of muxes started outside Conspectus, with no prompt scrubbing. Conspectus already reads comparably sensitive state, so that amends ADR 0086's Tier 3 rule rather than adding a sanitizer.

- Scope: per-kind records under `$XDG_STATE_HOME/conspectus/history/` (mux, checkout, repo, PR snapshot) with stable record ids, the common envelope, and kind recipes; presence (Live / Gone / Unknown) and the rule that absence from a scan never means gone; mux matching order (same incarnation, ghost with same key, new); writers, the advisory lock, commutative merge, fresh-probe-only rule, 10-minute `last_seen` cadence, read-only modes; ignore globs; manual reaping with per-kind thresholds; the `node_presence` map and historical relations.
- Tests: docs-only; `git diff --check`.
- Manual checks: none.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A new ADR under `docs/adr/` in the house style (status, context, decision, consequences, alternatives, open questions answered) is indexed in `docs/adr/README.md`
- [ ] #2 The ADR supersedes ADR 0058 and ADR 0102 and amends ADR 0057 (binding) and ADR 0086 (verbatim argv in Tier 3 records); each of those ADRs gets a status line pointing at it
- [ ] #3 The ADR covers record shapes per kind, presence rules per kind, mux matching order, writer coordination and merge rules, cadence, read-only modes, ignore rules, and reaping thresholds
- [ ] #4 Alternatives considered include storing history in `graph.bin`, name-only mux identity, a single elected discovery writer, automatic reaping, and scrubbing prompts from argv
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
