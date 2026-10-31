---
id: CSP-544
title: Clean up dependency and tooling leftovers
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
updated_date: '2026-10-31 08:16'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 615000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Progress (2026-09-30, `CSP-557` / `CSP-561`): `proptest`,
  `rstest`, `_selection_display`, and the "CSP-088 wave N" comments
  are gone. Remaining: confirm `tui-pantry`, and resolve `pre-commit`
  without a config.
- Scope: `proptest` and `rstest` are dev-dependencies with zero uses;
  `tui-pantry` serves only `examples/pantry.rs` (confirm the `CSP-424`
  "go" still holds); the flake ships `pre-commit` but there is no
  `.pre-commit-config.yaml` (ADR 0007 promised hooks); dead helper
  `_selection_display` (`src/cli/mod.rs:769`); historical
  "CSP-088 wave N" comments in `src/cli/mod.rs`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-013`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `examples/pantry.rs` now previews the framework's `tuikit::checklist::Checklist` and the host's Controls adapter (`conspectus::tui::widgets::controls`, a `tuikit::settings_panel` instance), and still calls itself a spike. ADR 0152 item 5 keeps `tui-pantry` a dev dependency and says the example moves with the controls it previews; it has not moved, since it imports host modules. The CSP-424 go/no-go stands: keep it and decide where it lives (a framework example would need `tui-pantry` in the framework's dependency allowlist in `crates/tuikit/tests/independence.rs` and a record in the framework's decision log), or retire the example and the dev dependency. The runtime evaluation (`docs/plans/tui-framework/runtime-evaluation.md`) notes the example runs on `tui-pantry`'s loop, not Conspectus's.
<!-- SECTION:NOTES:END -->
