---
id: CSP-331
title: Recall focus on deep-link entry
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-transcript
  - wont-do
milestone: m-11
dependencies: []
ordinal: 206000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Won't fix on the conspectus side** as of ADR 0052 — the native viewer owns its own focus model. Kept in the backlog for operators who configure recall as their viewer via `CSP-332`.

- Scope: when patched-recall is launched with `--session <id>`,
  the operator has already chosen the session in conspectus
  and lands inside recall expecting to scroll. But recall has
  no modal focus model — every keystroke other than the
  explicit navigation keys (Up/Down/Ctrl-E) inserts into the
  search query box. The most ingrained muscle memory
  (`j`/`k` to move down/up) ends up filtering the session
  list instead, often hiding the very session conspectus
  just opened.
- Resolution options:
  (a) **Extend the conspectus recall patch** with a deep-link
      mode: when `--session` is set, intercept `j`/`k` (and
      possibly `g`/`G`) as preview-navigation keys before
      they reach `on_char`. Keep `/` as the escape hatch to
      re-enter search.
  (b) **Upstream PR** for a `--preview-focus-on-entry` flag
      (or a deeper modal-focus rework) — recall would benefit
      from this regardless of conspectus's use case.
  (c) **Status-bar hint after launch** explaining the
      Up/Down convention — cheapest, no patch revision.
- Tests: hard to unit-test the patched binary directly; rely
  on the existing recall installCheck plus a one-line manual
  smoke step in the CSP-216 outcome notes.
- Blockers: none. Pairs naturally with `CSP-330` —
  both are recall-patch revisions.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-015`
