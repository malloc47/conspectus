---
id: CSP-562
title: >-
  Recover poisoned locks in the daemon with
  `lock().unwrap_or_else(PoisonError::into_inner)` instead of ten hand-written…
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 571000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Recover poisoned locks in the daemon with `lock().unwrap_or_else(PoisonError::into_inner)` instead of ten hand-written matches (`27f33de`).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RUST-009`
