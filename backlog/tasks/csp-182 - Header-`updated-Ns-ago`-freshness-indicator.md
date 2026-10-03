---
id: CSP-182
title: Header `updated Ns ago` freshness indicator
status: Done
assignee: []
created_date: '2026-05-19 23:23'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 409000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Msg::SetData` now carries `loaded_at_epoch`,
`App::loaded_at_epoch()` exposes it, and the header renders
`updated Ns ago · counts` via the shared
`format_recency` helper. Render-time clock is a
`#[cfg(test)]`-controllable shim so snapshot tests stay
deterministic.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-005`
