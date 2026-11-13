---
id: m-26
title: "Live Agent State"
---

## Description

Show what each running agent session and mux is doing now, in the operator's three buckets: waiting on them (permission, question, plan, input), ready for a new prompt, and working (generating, running a tool, retrying, compacting, with subagents). Plus error, starting, exited, and unknown.

Scoped inside-out from `docs/plans/live-state/` (six research notes: Claude Code, Codex, OpenCode, Pi, other orchestrators, and an internal audit). ADR 0203 records the neutral vocabulary and effective-state rules, ADR 0204 the per-session store (outside the attribution spool, no daemon on the hook path), ADR 0205 the `node_live` snapshot side table and binding, ADR 0206 the minimal event set per harness and what a writer may keep, and ADR 0207 the rule that hook-free sources (pane title, `claude agents --json`, transcript tails) each need their own decision.

Order of work: vocabulary, side table, and store first; then the fast silent writer and the Claude Code vertical slice; then binding, CLI surfaces, and TUI plumbing that carries state onto the row models without rendering it; then Codex, OpenCode, and Pi; then the attribution-path fixes the research found and the operator guide. Manual verification against real harnesses closes phase 1.

Parked on operator decisions (label `needs-decision`): everything the operator sees in the TUI (cell and glyph, mux rollup, header chip, ordering, jump key, seen/unseen, detail pane, notifications) and each hook-free source. `docs/plans/live-state/tui-options.md` holds the options and a recommended default for each.

All tasks carry the `live-state` label plus one area label: `ls-model`, `ls-store`, `ls-emit`, `ls-bind`, `ls-cli`, `ls-tui`, `ls-fix`, `ls-docs`, or `ls-policy`. CSP-268 is superseded by this milestone.
