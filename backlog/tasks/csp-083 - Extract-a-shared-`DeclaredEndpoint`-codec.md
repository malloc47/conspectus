---
id: CSP-083
title: Extract a shared `DeclaredEndpoint` codec
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies: []
ordinal: 83000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`91963c8`). `parse_endpoint` +
  `endpoint_label` retired from cli.rs; replaced by
  `DeclaredEndpoint::parse_compact` / `compact_label`
  methods. Round-trip tests for all 10 variants + syntax
  error + missing-field error tests in `declared_tests.rs`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-001`
