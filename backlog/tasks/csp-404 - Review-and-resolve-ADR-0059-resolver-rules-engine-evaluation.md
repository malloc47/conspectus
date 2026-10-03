---
id: CSP-404
title: Review and resolve ADR 0059 (resolver rules-engine evaluation)
status: Done
assignee: []
created_date: '2026-06-08 21:23'
labels:
  - h-adr
milestone: m-11
dependencies: []
ordinal: 114000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: read `docs/adr/0059-resolver-rules-engine-evaluation.md`,
  decide accept / amend / reject. Key knobs to tune if accepting:
  (a) the deferred-Ascent posture in §Decision (3), (b) the
  counted-bug re-trigger threshold in §Decision (4). Update status
  from `Proposed` to `Accepted` / `Rejected` / `Superseded` and
  record any amendments inline. Skipping accept-as-drafted is fine;
  the artifact's purpose is to stop the question from re-surfacing
  without an explicit re-trigger.
- Tests: none (ADR-only).
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0059 accepted as drafted. The resolver stays in Rust;
`CSP-096` implements typed score breakdowns rather than adopting a
rules engine. The Ascent re-trigger remains "three or more
derivation-pass bugs after `CSP-096` ships."
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-ADR-0059-REVIEW`
