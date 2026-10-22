---
id: CSP-385
title: Glob/wildcard pin patterns
status: Done
assignee: []
created_date: '2026-06-04 21:32'
updated_date: '2026-10-22 15:06'
labels:
  - h-pin-f
milestone: m-11
dependencies: []
ordinal: 334000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- `[[pins.patterns]]` surface synthesizing ephemeral pins from
  path globs (`~/work/*` → one codex pin per matched checkout),
  modeled on sesh's `[[wildcard]]` table.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-F-004`

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Obsolete: pin slots (ADR 0141) replace pin entries, and the plan's decisions rule out wildcard pins; no work remains.
<!-- SECTION:FINAL_SUMMARY:END -->
