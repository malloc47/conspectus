---
id: CSP-184
title: Move TUI discovery onto a background thread with timer-driven refresh
status: Done
assignee: []
created_date: '2026-05-19 23:23'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 444000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Landed across four incremental waves**, closed by
  `d513994` on 2026-07-05:
    - Background worker + mpsc plumbing (`spawn_discovery_worker`
      + `DiscoveryResult` channel + non-blocking `drain`).
    - Timer-driven auto-refresh (`refresh_interval` gate
      + `pending_refresh` in-flight flag).
    - `Action::Refresh` on `r` shares the same async spawn
      path.
    - `Msg::SetRefreshFailure` posts a status message and
      preserves the last-good snapshot on error.
    - Selection retention across refresh via the reducer's
      `Msg::SetData` handling (per CSP-165).
    - Provider diagnostics populate through
      `populate_provider_status` on both init and each drain.
    - Wave closer (`d513994`): `LiveMode::init` now spawns
      the initial discovery on the same background worker
      path instead of blocking the first frame draw. Startup
      input latency drops from "discovery-scan-time" (~200-
      800ms on real repos) to ~zero; the operator sees the
      frame paint immediately with the pre-existing "Loading
      discovery…" placeholder while the worker runs.
- Shape lets a Phase 7 daemon snapshot transport swap in as a
  peer `DiscoveryResult` producer without touching `app.rs`.
- Blockers: none (retroactively cleared).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-007`
