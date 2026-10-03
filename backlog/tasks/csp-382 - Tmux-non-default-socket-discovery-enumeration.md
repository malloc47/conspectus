---
id: CSP-382
title: Tmux non-default socket discovery enumeration
status: To Do
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin-f
milestone: m-11
dependencies: []
ordinal: 331000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Extend the tmux runner to scan
  `{default} ∪ {pin.mux.socket_name | active pin}` so non-default-
  socket pins become bindable. Decide whether to expose a
  `[tmux] sockets = [...]` config knob for sockets without an
  owning pin.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-F-001`
