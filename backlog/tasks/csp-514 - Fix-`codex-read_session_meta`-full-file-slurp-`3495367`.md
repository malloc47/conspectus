---
id: CSP-514
title: 'Fix `codex::read_session_meta` full-file slurp (`3495367`)'
status: Done
assignee: []
created_date: '2026-07-29 02:07'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 538000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Was `fs::read_to_string` on JSONL rollouts up to 24 MB to look at the first line; replaced with `BufReader::read_line`. **167 MB/s → 37 MB/s** — the single biggest win in the chain, found in ~30 seconds of `sudo strace`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-006`
