---
id: CSP-575
title: Launches keep failed panes and confirm the start (ADR 0103)
status: Done
assignee: []
created_date: '2026-10-01 18:04'
labels:
  - h-pin-fix
milestone: m-11
dependencies: []
ordinal: 343000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `remain-on-exit failed` on Conspectus-created sessions;
  `MuxBackend::pane_status`; watch the new pane after `pin launch` /
  `mux launch`; resume falls back to fresh on a failed start and
  clears the sidecar; replace a stale pin's dead pane instead of
  `send-keys`; `capture-pane -S -100`.
- Tests: `cli::launch_watch` (started, unobservable, resume
  fallback, failed fresh launch, vanished session, output tail);
  manual end-to-end on a scratch socket with a harness that rejects
  `--resume`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus-worker1`, whose sidecar pointed at a
turnless transcript, now resumes, fails in about a second, and
launches fresh with the harness's message on stderr. Previously
the only symptom was "can't find session".
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-FIX-002`
