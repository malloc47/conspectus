---
id: CSP-418
title: Audit the sessions-pane header content holistically
status: Done
assignee: []
created_date: '2026-06-17 15:15'
labels:
  - h-ui
milestone: m-11
dependencies:
  - CSP-415
ordinal: 358000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: review every span the left-pane header
  (`src/tui/ui.rs:left_panel_title` + `append_header_chips`)
  renders today — the freshness chip, view-tab strip,
  `N agents · M mux` counter, per-harness chips, and the
  three-bucket `◉ / ◐ / ◯` mux-state chip section — and
  decide what each one is actually paying for. The motivating
  questions:
    - Does the global three-bucket mux chip section still
      carry weight now that the per-row chip is binary
      (ADR 0072) and group rows own ambiguity? If "find an
      ambiguous session" is the use case, is a filter affordance
      the better answer?
    - Are per-harness counts duplicating signal the harness
      badges + group summaries already provide?
    - Does the view-tab strip stay in the header or move to a
      dedicated row so the header can shrink to one line on
      narrow terminals?
    - Should the freshness / refresh state move into the
      status bar so the header carries identity + counts only?
  The deliverable is a short design note (or ADR if the
  decisions reach across surfaces) plus the implementation
  that drops or relocates whatever the audit decides is
  redundant. Pre-commit to nothing — the audit might choose
  "keep everything, just tidy the placement."
- Tests: header snapshot coverage at wide / mid / narrow
  widths after each chip removal or relocation; coverage for
  the chip-section drop / re-introduce path under filter and
  no-filter states.
- Open questions: whether the header redesign should also
  cover the `mux` / `union` / `prs` / `forks` views (their
  headers re-use the same composition) or scope strictly to
  sessions; whether the per-harness chips become an opt-in
  `--show-harness-chips` flag instead of always-on.
- Blockers: `CSP-415` landed (the binary chip is the trigger
  for re-evaluating the header chips); coordinate with
  `CSP-416` so any new glyph language doesn't get rewritten
  twice.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-20): audit landed in three commits +
ADR 0078. Per-element verdict + width snapshots captured
in `docs/plans/sessions-header-audit.md` (`53767ac`);
implementation in `8360768` rewrites `draw_header` per the
seven decisions the operator approved
(drop brand + `sessions` view-label words, switch
`N of M agents` → `N/M sessions`, add `[tui]
show_harness_chips` opt-in, collapse three-bucket mux
chips to `⚠ N` only when N > 0, keep freshness in
header, apply uniformly across views); ADR 0078
(`b8922ff`) memorializes the cross-surface rubric
(header carries freshness + load-bearing counts + opt-in
aggregates + actionable triage chips; row tree carries
per-row signals; status bar carries focus + view-state
chips + contextual hints + transient toasts) for future
chrome work.
Width reclaimed: header drops from ~150 cells at default
to ~42 cells; pre-audit even the bare prefix overflowed at
70 cols, the post-audit header fits at 50 cols with room
to spare. 1667 tests pass byte-identical; the two
pre-audit chip tests retired (their behavior no longer
exists), replaced by three post-audit tests
(default-drop, narrow-fit, opt-in shows chips).
Deferred: per-view count language (mux view still says
`sessions`), mobile-narrow layout (< 40 cols), status-bar
evolution under CSP-423 — all recorded in ADR 0078's
"Open Questions Deferred" section.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-004`
