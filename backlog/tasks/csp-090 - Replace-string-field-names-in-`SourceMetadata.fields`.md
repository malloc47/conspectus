---
id: CSP-090
title: Replace string field names in `SourceMetadata.fields`
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies:
  - CSP-085
ordinal: 90000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`4b3d816`). New `crate::model::source_field`
  module with constants MATCH_KIND, MUX_ACTIVITY_EPOCH,
  UPDATED_EPOCH, FORK_ROOT, LINEAGE_KIND, STATE, IS_DRAFT,
  LOGICAL_PATH, SCOPE. Migrated 13 files across discovery,
  resolve, and output layers to use the constants for both
  `.insert("<key>".to_string(), ...)` producers and
  `.get("<key>")` consumers. `every_source_field_constant_matches_its_string_literal`
  test guards against constant renames.
- Blockers: `CSP-085` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-008`
