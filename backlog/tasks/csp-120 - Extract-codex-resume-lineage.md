---
id: CSP-120
title: Extract codex resume lineage
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-lineage
milestone: m-11
dependencies: []
ordinal: 183000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Resolution: real codex rollouts (cli 0.128) expose
  `session_meta.payload.forked_from_id`, a true fork pointer (multiple
  children can share one parent). The codex adapter now extracts that
  field and emits a `parent_session` candidate per child rollout;
  resolved when the parent rollout is on the same state root, otherwise
  `UnresolvedEndpoint` evidence with `harness_key = "codex"` and the
  parent native id. `lineage_kind = "fork"` per ADR 0018. Codex does
  not currently expose a separate resume-only pointer (resume continues
  writing into the same rollout file), so resume lineage is parked
  until codex publishes a distinguishable field — no upstream issue
  filed yet; reopen this item if codex changes the rollout format.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-LINEAGE-004`
