---
id: CSP-556
title: Look up nodes without cloning their ids (`30f1f7d`)
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 565000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`GraphNode::has_id` and `GraphSnapshot::find_node` replace 14 `find(|n| n.id() == *target)` scans that allocated an owned `NodeId` per node.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RUST-003`
