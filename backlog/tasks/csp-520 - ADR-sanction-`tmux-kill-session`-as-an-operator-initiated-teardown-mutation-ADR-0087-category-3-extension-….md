---
id: CSP-520
title: >-
  ADR: sanction `tmux kill-session` as an operator-initiated teardown mutation
  (ADR 0087 category-3 extension) +…
status: Done
assignee: []
created_date: '2026-08-05 02:51'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 557000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR: sanction `tmux kill-session` as an operator-initiated teardown mutation (ADR 0087 category-3 extension) + `MuxBackend::kill_session`. **ADR 0093 Accepted.** Teardown is two-phase (graceful `SIGTERM` → configurable grace → hard `kill-session`); confirmation is configurable via `[worktree] teardown_confirm = always|live|never` (default `live`) and `teardown_grace` (default `3s`), with CLI-flag overrides. The `kill_session` backend method + the graceful orchestration land with CSP-522 (close-down).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-ENV`
