---
id: CSP-491
title: Replace "read-only first" with a defined mutation envelope
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-adr
milestone: m-11
dependencies: []
ordinal: 176000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New tenet ADR 0087 defines the mutation
  envelope: four sanctioned write categories (user-intent
  TOML stores, rebuildable observation sidecars under
  `$XDG_STATE_HOME/conspectus/`, operator-initiated mux
  lifecycle including the narrow pin-launch `send-keys`
  exception on Conspectus-constructed argv, and
  Conspectus-owned subprocess launches) and seven absolute
  prohibitions (harness-native state, terminal input into
  live agent panes, payload persistence, shared/system
  locations, background mutation, git-state mutation, hook
  bypass). CLAUDE.md's "start read-only unless" bullet
  replaced with a citation-and-summary of ADR 0087.
  docs/design.md's Initial Scope section extended with an
  inline pointer, and ADR 0087 added to the Decisions
  catalog alongside ADR 0086. No code change — the envelope
  describes decisions already in the codebase.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-ADR-002`
