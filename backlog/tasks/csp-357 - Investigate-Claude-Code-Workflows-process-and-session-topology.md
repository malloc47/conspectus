---
id: CSP-357
title: Investigate Claude Code Workflows process and session topology
status: To Do
assignee: []
created_date: '2026-06-03 22:58'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 267000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: audit Claude Code Dynamic Workflows
  (`https://code.claude.com/docs/en/workflows`) against Conspectus's
  mux/process/session attribution model. The docs say workflow runs
  execute in the background while the parent session stays responsive
  and can orchestrate many agents. Determine, from live process
  trees and Claude state files, whether a workflow keeps one
  foreground controlling Claude PID with background implementation
  processes, launches one process per workflow agent/session, or
  uses another topology. Record how `/workflows`, task-panel
  expansion, pause/resume, and stopped/restarted agents affect
  `~/.claude/sessions/*.json`, transcript files, hook records, PIDs,
  parent PIDs, and process start identities.
- Current assumption: until this story lands, Conspectus continues
  to treat one live controlling foreground harness PID as one
  human-driven agent session. Claude background/spare/workflow
  implementation processes should not inflate mux cardinality or
  appear as human-attached mux processes.
- Tests: fixture the observed workflow state shape once audited,
  including at least one active workflow with multiple agents and
  one paused/resumed run. Add process-linking tests for whichever
  topology is confirmed.
- Manual checks: run a small `/deep-research` or saved workflow in a
  disposable tmux-backed Claude session, capture `tmux list-panes`,
  `ps --forest`, `~/.claude/sessions/*.json`, workflow scripts under
  `~/.claude/projects/`, and Conspectus graph output before, during,
  after pause/resume, and after completion.
- Blockers: access to Claude Code v2.1.154+ with workflows enabled.
- Related: `CSP-249`, `CSP-227`, ADR 0028.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-019`
