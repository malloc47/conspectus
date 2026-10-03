---
id: CSP-079
title: File the Atelier-side delegation work in the Atelier repo
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-076
  - CSP-077
ordinal: 74000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: open the cross-repo tracker covering Atelier's deprecation
  or delegation of `atelier session list`, `atelier mux status`,
  forge status, and the graph-heavy parts of `atelier status`. The
  code lives in the Atelier repo; this item is purely outbound
  coordination, including pointing Atelier at `CSP-076`'s curated
  API and `CSP-077`'s migration guide. Cite the Atelier issue or PR
  URL in the outcome note so future readers can follow up.
- Tests: none (out-of-repo work).
- Manual checks: confirm an Atelier maintainer (or self, if dual
  maintainer) has accepted the tracker.
- Blockers: `CSP-076`, `CSP-077`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Atelier-side tracker in
`/home/user/src/atelier/docs/conspectus-delegation.md` and
linked it from Atelier docs in commit `b765c16` (`docs: track
Conspectus delegation work`). The tracker points Atelier at
Conspectus commits `46d31ac`, `652dd43`, and `49f170d`, covers
`atelier session list`, `atelier mux status`, `atelier pr status`,
and graph-heavy `atelier status` areas, and records acceptance
criteria for preserving existing workflows.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-007`
