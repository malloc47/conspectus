---
id: CSP-471
title: 'Finish the `output::render` migration and settle `dev_scenarios` gating'
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 102000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Landed 2026-07-05** (commit `b07c1c6`) at the corrected
  scope. The story's original framing ("delete the shim,
  move everything to render.rs") wasn't achievable —
  `output/render.rs` enforces a
  `substrate_has_no_model_deps` invariant test that would
  trip on `render_with` / `render` / `indicator` /
  `node_short_id` because they take `GraphSnapshot` /
  typed `Provenance` / typed `Confidence`. table.rs is
  actually the projection dispatch layer outside the
  substrate, not a shim. Landing shape:
  * Module header rewritten from "re-export shim" to
    "projection dispatch"; substrate boundary called out
    inline so future readers don't repeat the
    investigation.
  * 27-item `pub use super::render::{...}` block deleted.
    Substrate items now reached through `output::render::*`
    directly.
  * ~20 caller migrations across `cli.rs`, `dev_scenarios.rs`,
    `tui/rows/sessions.rs`, `output/table_tests.rs`,
    `tests/pins_snapshots.rs`, `tests/declared_snapshots.rs`.
  * `dev_scenarios` gating: the module is already
    `#[cfg(any(test, debug_assertions))]` in `lib.rs:7` and
    the CLI dispatch is `#[cfg(debug_assertions)]` in
    `cli.rs:142`. Release builds already exclude it. Noted
    as the intended ship shape without introducing a
    dedicated feature flag.
- Net table.rs shrink: 149 → 111 prod lines (−38 lines of
  dead re-exports). Every downstream import now names its
  canonical source module.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-010`
