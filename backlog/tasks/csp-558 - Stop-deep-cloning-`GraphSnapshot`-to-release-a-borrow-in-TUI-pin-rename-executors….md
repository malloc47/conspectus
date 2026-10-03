---
id: CSP-558
title: >-
  Stop deep-cloning `GraphSnapshot` to release a borrow in TUI pin/rename
  executors…
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 567000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Stop deep-cloning `GraphSnapshot` to release a borrow in TUI pin/rename executors; derive `Clone` for `GraphDb` (`e15ea7d`).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RUST-005`
