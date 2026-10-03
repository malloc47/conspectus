---
id: CSP-408
title: 'ADR: node-type visual identity system'
status: Done
assignee: []
created_date: '2026-06-09 14:32'
labels:
  - h-vis
milestone: m-17
dependencies: []
ordinal: 520000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0073 records the strategy (geometric default,
Nerd-Font opt-in via `[tui.theme.icons]`), the per-kind slate
(`▦ ◆ ◇ ● ▣ ⚙ ⎇ ⑂ ⇄`), the placement rule (one-cell prefix
between disclosure and existing badges), the color schema
(eight new `node_*` color keys, `ForgePr` reuses `pr_*`), and
the accessibility stance (1-cell-only override validation,
glyph-only legibility under `NO_COLOR`). `AgentSession` keeps
an independent kind-color layered with the harness pill.
Subsequent stories (`CSP-409..413`) implement the slate.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIS-001`
