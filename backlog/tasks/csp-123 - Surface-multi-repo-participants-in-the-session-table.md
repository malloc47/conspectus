---
id: CSP-123
title: Surface multi-repo participants in the session table
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-agentmux
milestone: m-11
dependencies: []
ordinal: 233000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`output::agent::fetch_workspace_lookup` joins the
resolver's chosen `workspace_contains_repo` selections and
renders the workspace column as `atelier+conspectus`-style
`+`-joined member basenames when ≥2 distinct members exist;
single-repo workspaces keep the root path. Opt-in only via
`--columns ...,workspace,...` — the default `SESSIONS_COLUMNS`
set is unchanged. Promoting `workspace` into the defaults is
deferred per ADR 0060 §Alternatives. Atelier multi-repo
workspaces exercise the same surface from day one.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-AGENTMUX-003`
