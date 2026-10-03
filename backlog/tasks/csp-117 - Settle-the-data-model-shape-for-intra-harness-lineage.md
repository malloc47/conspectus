---
id: CSP-117
title: Settle the data-model shape for intra-harness lineage
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-lineage
milestone: m-11
dependencies: []
ordinal: 180000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Resolution: ADR 0018 extends ADR 0005 to allow intra-harness
  `parent_session` / `child_session` candidates to attach directly
  between two `AgentSession` endpoints. `lineage_kind` is standardized
  as the operation vocabulary (`compaction`, `resume`, `fork`,
  `fresh`, `unknown`); attribution fidelity moves to a separate
  `lineage_fidelity` field, which Atelier will adopt in CSP-118.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-LINEAGE-001`
