---
id: CSP-224
title: Define Conspectus hook sidecar schema and trust/ranking rules
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-223
ordinal: 261000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: if `CSP-223` finds viable hook/plugin emitters,
  define a provider-neutral sidecar record written outside project
  trees, likely under the user's XDG state directory. Minimum
  candidate fields: harness key, session key, cwd, pid, ppid,
  tmux pane id, tmux socket/session/window/pane metadata,
  transcript/session path, hook event kind, observed timestamp, and
  harness version. Define stale-record expiry, collision handling,
  privacy expectations, and evidence ranking. Proposed ranking:
  explicit hook session id + tmux pane/pid above open-fd evidence;
  hook transcript/session path above open-fd evidence when the path
  resolves to a discovered session; hook cwd-only records below
  command session ids and above generic cwd matching only when the
  timestamp is fresh.
- Tests: schema parse/round-trip tests, stale-record filtering,
  duplicate event coalescing, malformed record degradation, and
  resolver ordering tests against fd, command, and cwd evidence.
- Blockers: `CSP-223`; ADR required for the durable sidecar
  convention.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0028 defines schema version 1 under the user's
Conspectus state directory, a 15-minute active-record TTL,
matching by explicit session id plus tmux native id / pid / cwd,
and ranking above launch argv evidence.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-010`
