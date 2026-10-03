---
id: CSP-217
title: Add read-only session-file activity correlation
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-136
ordinal: 246000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: improve fresh-session attribution without sending input to
  running agents. Add a read-only observation layer that correlates
  tmux pane PIDs with harness session files by recent creation /
  modification / open activity. Preferred implementation is a
  Linux-first provider that can consume `inotify`/fanotify-style
  events in continuous mode and a one-shot fallback that scans
  known harness state roots for recently-created or recently-
  modified session files matching the pane cwd and harness. Treat
  this as weaker than an open fd match but stronger than cwd-only
  matching when the event/file timestamp is close to the pane
  process start time. Do not write marker files and do not touch
  transcript contents.
- Tests: fixture-backed clocked tests covering Codex rollout file
  creation, Claude project/task file creation, opencode sqlite
  session updates, stale files outside the time window, and
  ambiguous same-cwd events that remain candidates instead of
  becoming a single preferred link.
- Manual checks: start a fresh harness session in tmux with no
  explicit resume id; confirm `graph --format json` gains a
  non-cwd `LinkedToMux` candidate after the session file appears.
- Blockers: `CSP-136`; friendlier after the continuous-mode
  snapshot workstream starts.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
One-shot discovery now correlates active-pane harness
identity, mux cwd, and read-only harness session activity
timestamps already collected from Codex rollout files, Claude
transcript files, and opencode session state. Fresh same-harness /
same-cwd sessions near mux creation or activity emit
`session_file_activity_match`, ranking below fd/hooks/state but
above process, launch argv, and cwd-only evidence. When process
cardinality shows zero or one non-subagent harness process, fresh
same-cwd activity candidates collapse to one human session; multiple
attributions remain possible only when multiple harness processes
are observed. Inotify / fanotify continuous-mode event ingestion
remains deferred to the continuous server workstream.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-003`
