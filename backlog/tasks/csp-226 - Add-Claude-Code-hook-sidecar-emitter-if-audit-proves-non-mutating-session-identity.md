---
id: CSP-226
title: >-
  Add Claude Code hook sidecar emitter if audit proves non-mutating session
  identity
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-223
  - CSP-224
ordinal: 263000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: if Claude Code hook payloads include a current session id,
  transcript path, or enough context to derive one, provide a
  minimal documented hook command/script that writes Conspectus
  sidecar records. Prefer explicit hook payload fields over
  inspecting parent process fds. The hook must be opt-in and easy
  to remove; Conspectus should discover its records but not require
  users to install it.
- Tests: payload fixture tests for supported Claude Code hook
  events; sidecar record generation tests; version/field-missing
  degradation.
- Manual checks: enable the hook for a live Claude Code session and
  verify the sidecar identifies the active session without adding
  Conspectus probe messages to JSONL logs.
- Related: `CSP-227` provides the concrete failure mode this
  emitter should fix if Claude Code hook payloads expose the
  post-`/resume` current session id.
- Blockers: `CSP-223`, `CSP-224`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `scripts/conspectus-claude-hook-sidecar.py` and
documented a `SessionStart` hook configuration in
`docs/operations.md`. The emitter writes schema-v1 sidecar
records with Claude `session_id`, `transcript_path`, `cwd`,
process ids, and tmux context when available. The script is now a
compatibility shim for `conspectus hook write claude-code`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-012`
