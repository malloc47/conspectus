---
id: CSP-581
title: >-
  Related view at neighbor granularity; split corroborating from competing
  candidates (ADR 0107)
status: Done
assignee: []
created_date: '2026-10-02 19:18'
labels:
  - h-ui
milestone: m-11
dependencies: []
ordinal: 363000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Symptom (2026-10-02): a mux's `Related` section listed
  `attached session <S>` as validated and the same session again,
  marked `⚠`, under `Other`. On the live graph both resolved
  `LinkedToMux` slots had a pin winner plus a hook-sidecar or
  `cross_link` candidate for the same mux, reported as competing.
- Cause: the explorer renders one row per candidate link, and
  `resolve_links` puts every non-winner in `competing_link_ids` (and
  emits `Diagnostic::Conflict`) even when it names the winner's
  target.
- Scope:
  - Resolver: add `ResolvedRelationship.corroborating_link_ids`
    (same target as the winner) and keep `competing_link_ids` for
    different targets. Emit `Diagnostic::Conflict` only for
    different-target competitors. No-winner slots (ADR 0077) fold
    corroborating ids into competing. Add
    `ResolutionExplanation.corroborating`. Bump
    `snapshot::FORMAT_VERSION`.
  - Explorer: fold each group's links by neighbor into one row with
    an `evidence` list; classify the row `Resolves` / `Conflict` /
    `AltOf` from its links. Key `ValidatedLink` / `OtherLink` rows by
    `(direction, relation, neighbor)`.
  - Preview zone: list every backing link under `evidence`.
  - `conspectus node` and the HTML inspector show corroborating ids
    separately from competing ones.
- Tests: resolver same-target vs different-target slots, conflict
  diagnostic only on disagreement, no-winner slot folding; explorer
  pin + hook for one mux yields one validated row and no Other zone,
  a different-target competitor still lands in Other as `Conflict`,
  row keys survive a representative-link change; TUI snapshot of the
  preview `evidence` list.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-10-02): resolver split landed with a
`FORMAT_VERSION` bump to 4; same-target pin/declared/hook
candidates in the pin, declared, and HTML snapshots moved from
competing to corroborating, and their conflict diagnostics went
away. The branch→PR resolver tests had modelled PR→branch with
identical endpoints; they now use the real branch→PR direction
with distinct PRs so they still exercise competition. Explorer
rows fold by neighbor with an `evidence` list, shown in the
Preview zone as `<evidence kind or adapter> · <provenance> ·
<confidence>`. On the live graph `tmux:conspectus-main` now reads
`3 validated` with one `attached session` row and no Other zone.
`tui --snapshot` couldn't move the explorer cursor because its key
driver skips the focus remap (`CSP-582`), so the preview is covered
by a UI test instead.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-009`
