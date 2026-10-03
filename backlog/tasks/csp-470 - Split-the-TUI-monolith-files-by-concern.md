---
id: CSP-470
title: Split the TUI monolith files by concern
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 101000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Waves 1–2 landed 2026-07-05**: * Wave 1 (`2dd21d6`): extracted `tui/ui.rs`'s 3,378-line `mod tests` to sibling `tui/ui_tests.rs` via `#[path = "ui_tests.rs"] mod tests;`. `ui.rs` shrunk from 6,674 → 3,296 lines. Test module still lives at `tui::ui::tests` at compile time so `use super::*;` resolves unchanged. * Wave 2 (`65c7a85`): same shape for `widgets/pins.rs` — extracted 1,498 test lines to sibling `pins_tests.rs`. `pins.rs` shrunk from 4,266 → 2,770 lines. * The story's original scope was a view/panel split by "3 `match view` sites", but a survey turned up only 1 dispatch (`view_kind_key`) — the natural boundary the story assumed doesn't exist. Test extraction is the highest-value split available and lands the same "reviewable smaller files" outcome without a synthetic view partition. **Wave 3 (widgets/pins.rs production split)** stays deferred to `CSP-497`: the model/render split needs the modal stack contract to supply the split axis (a state-machine boundary between pin-menu selection logic and per-sub-editor form rendering). Wave-2's test extraction already addresses the "reviewable smaller files" outcome the story primarily wanted; the additional prod-side split is CSP-497 territory and lands there.

- Scope: `tui/ui.rs` (~3.2k production lines) splits by view/panel —
  dispatch is already centralized in 3 `match view` sites so extraction
  is clean; `widgets/pins.rs` (~2.8k) separates the pins menu model
  (actions, selection-aware defaults; unit-testable without ratatui)
  from form rendering, targeting the `CSP-497` Overlay contract as
  the split boundary.
- Tests: `conspectus tui --snapshot` runs and TUI snapshot suites
  byte-identical.
- **Sizing note (2026-07-04)**: 6k+ lines of file moves across two
  files. Land as a dedicated multi-commit series to keep each
  landing reviewable: (a) `tui/ui.rs` view-panel split (sessions,
  mux, prs, forks, union each as its own module); (b) `widgets/pins.rs`
  model / render split; (c) sibling-file `tests.rs` extraction for
  any large `mod tests` blocks that carry along. Snapshot suites are
  the regression net between waves.
- Blockers: none; independent of `CSP-498`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-009`
