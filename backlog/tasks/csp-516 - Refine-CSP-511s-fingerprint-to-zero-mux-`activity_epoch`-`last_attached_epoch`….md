---
id: CSP-516
title: >-
  Refine CSP-511's fingerprint to zero mux `activity_epoch`,
  `last_attached_epoch`…
status: Done
assignee: []
created_date: '2026-07-29 02:07'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 540000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Refine CSP-511's fingerprint to zero mux `activity_epoch`, `last_attached_epoch`, and agent `last_active_epoch` (`70becc5`). Without this the gate never closed on any operator box with a live tmux session.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-008`
