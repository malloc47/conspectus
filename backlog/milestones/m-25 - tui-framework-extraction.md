---
id: m-25
title: "TUI Framework Extraction"
---

## Description

The TUI's widgets, design tokens, and interaction conventions are separated into a workspace member crate, `tuikit` (`crates/tuikit/`), that Conspectus depends on and that depends on nothing from Conspectus. ADR 0150 records the boundary, scope, and independence rules, ADR 0151 the control set and its 18 house rules, and ADR 0152 who owns the third-party crates. `docs/plans/tui-framework/` holds the inventory, control set, gap analysis, and workspace-impact notes behind them.

Scope: components and design tokens (layer 1) and interaction conventions (layer 2). The app runtime (layer 3) stays in Conspectus; one optional story evaluates extracting it. Internal reuse only: no publishing and no stable API. The crate must still document itself, guard its own independence, and be ready to move to its own repository at any time.

Consistency first: each extraction story unifies a control on the best variant and lists every visible change in its final summary under Regressions and Consistency wins, checked with `conspectus tui --snapshot` at a normal and a narrow width.

Order of work: cleanups (delete the legacy detail model, retire the hidden Union and Forks views, reproduce-then-fix the suspected defects) and the workspace conversion with its guardrails come first. Foundations follow, then conventions and text entry, choice and command controls, reading surfaces, data display, and chrome. Separation readiness (complete docs, a framework decision log, a separation rehearsal, Conspectus doc updates) closes the extraction work. The last story assesses the impact on the rest of the backlog.

Landing order matters because other work edits the TUI: every story is a small, reviewable change that leaves `main` releasable. The theme story waits for the aider removal workstream (m-24) so the harness palette adapter is written once.

All tasks carry the `tui-framework` label plus one area label: `tuifw-cleanup`, `tuifw-workspace`, `tuifw-foundation`, `tuifw-conventions`, `tuifw-text`, `tuifw-choice`, `tuifw-reading`, `tuifw-data`, `tuifw-chrome`, `tuifw-separation`, or `tuifw-close`.
