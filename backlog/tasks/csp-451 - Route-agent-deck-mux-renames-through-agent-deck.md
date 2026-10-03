---
id: CSP-451
title: Route agent-deck mux renames through agent-deck
status: To Do
assignee: []
created_date: '2026-06-24 02:52'
labels:
  - h-agentmux
milestone: m-11
dependencies:
  - CSP-124
ordinal: 235000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when a mux session is managed by agent-deck (detected via
  agent-deck's `state.db` profile state), the `rename` TUI shortcut
  (`R`) and `conspectus rename mux` should use agent-deck's native
  rename mechanism instead of `tmux rename-session`. Agent-deck is
  the source of truth for these sessions, and a direct tmux rename
  would desynchronize agent-deck's internal mapping. This requires
  (i) understanding agent-deck's rename interface (SQLite write,
  CLI subprocess, or HTTP endpoint — to be determined during
  implementation), (ii) adding a mutation seam to
  `discovery::agent_deck` mirroring the existing
  `TmuxRunner::rename_session` pattern, and (iii) extending
  `rename::plan_session_rename` / `rename::plan_mux_rename` so the
  lockstep path selects the agent-deck mutation when the mux
  session is agent-deck-managed rather than a bare tmux session.
  The existing lockstep rename contract (ADR 0029) continues to
  apply: agent-session alias writes happen concurrently with the
  mux-native rename. If the agent-deck rename fails, the alias is
  already written and the operator sees a status message.
- Blockers: `CSP-124` (must read agent-deck profile state
  first to identify which mux sessions agent-deck manages).
- Related: `CSP-234` (tmux rename seam), `CSP-236`
  (lockstep contract), ADR 0029 (alias + rename), ADR 0060
  (agent-deck workspace composition).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AGENTMUX-008`
