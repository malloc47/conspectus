---
id: CSP-721
title: >-
  Move design tokens to a generic `theme` module with an app-registered style
  map
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 09:49'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-718
  - CSP-720
references:
  - docs/adr/0032-tui-theme-config.md
  - docs/adr/0022-terminal-color-and-styling.md
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: enhancement
ordinal: 807000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`Theme` has 41 fields: 19 are generic tokens and 22 are Conspectus-domain (harness, mux, PR, node, edge, command badge, cwd, icons). Domain-named tokens are also used as generic roles (`link_id` is the mux-name colour, `mux_attached`/`mux_unmuxed` mean positive and dim), so the palette cannot move as it is. Controls bypass it: 20 raw `Color::` literals (17 in `form.rs`, which takes no theme at all), 21 raw `Modifier::DIM` sites, and about 20 hard-coded `REVERSED` cursor rows, while `theme.selection_active` is read in three places. The muted look has four tokens, 21 raw DIM sites, and 5 local `dim()` helpers. The key list is kept in three hand-synchronized places with a one-directional drift test (inventory slice 01).

Scope: Create `tuikit::theme` with generic role tokens (surface, accent, muted, inert, selection active and inactive, success, warning, error, info, link, match highlight, badge, panel focus accent) and an app-registered named-style map for domain roles. Move the `StyleSpec` grammar and the config table walk into it. Conspectus registers its harness, mux, PR, node, edge, cwd, and command-badge roles from an adapter. Collapse `placeholder`, `secondary_text`, `ghost`, `divider`, and `mux_unmuxed` into `muted` and `inert`, accepting the old config keys as aliases. Replace every raw `Color::` and `Modifier::DIM` in production code with a token, so forms honor `[tui.theme]`. Keep the default palette visually the same. Make the key table one source of truth.

TUI changes:
- Consistency wins: forms and the viewer follow `[tui.theme]`; one muted look instead of four tokens plus raw DIM; a single list of theme keys with a two-way drift test
- Regressions or behavior changes: forms lose their fixed White-on-DarkGray active-field fill and black-on-white cursor cell, which become theme tokens; an operator who set several of the five old muted keys differently sees them merged; the aliases keep old configs parsing

Tests and checks: Theme parse tests for every old key and alias. A test that fails on a raw `Color::` or `Modifier::DIM` in framework source. `conspectus tui --snapshot` under the default theme and under a custom `[tui.theme]` for the forms, Controls, and the left pane.

Blockers: m-24: the harness palette adapter is written once, after the aider adapter and the `harness_aider` alias are gone.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::theme` holds the generic role tokens, the `StyleSpec` grammar, the named-style registry, and the config table walk; Conspectus registers its domain roles from an adapter
- [ ] #2 The five muted tokens collapse to `muted` and `inert`, and every old key still parses as an alias with documented behavior
- [ ] #3 Production code in `tuikit` contains no raw `Color::` or `Modifier::DIM`, enforced by a test
- [ ] #4 The theme key list is defined once and a two-way drift test replaces the one-directional test
- [ ] #5 `docs/operations.md` documents the new keys and the aliases
- [ ] #6 Forms render from theme tokens and a custom `[tui.theme]` changes them
- [ ] #7 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #8 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #9 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Snapshot tool: let `--snapshot` open the three forms (new mux `n`, mux launch `N`, pin edit from the Pinned row menu) so before/after frames can be captured; capture default and custom-theme frames with that binary first.
2. Move the style-spec grammar (`StyleSpec`, `parse_color`, `parse_modifier`, `parse_style_spec`) into `tuikit::theme` with its tests; tuikit gains `ratatui` (on its allowlist); the host re-exports it.
3. Add `tuikit::theme::Theme`: typed generic tokens declared once by a macro (field, key, kind, default, description), an app-registered named-style map, alias keys with a fixed precedence, and the config table walk over host-converted values. Extend the grammar with `fg on bg`. Add the raw `Color::`/`Modifier::DIM` guard test in tuikit.
4. Host adapter `src/tui/theme.rs`: wrap the framework theme (Deref) plus icon overrides; register harness, mux, cwd, command badge, PR, node, edge roles and the legacy aliases (placeholder/divider/mux_unmuxed -> muted; secondary_text/ghost -> inert; link_id -> link; panel_focus_accent -> accent; harness_* -> harness.<key>). Config loader uses the walk. Call sites move to the new tokens and role accessors.
5. Replace the remaining raw `Color::`/`Modifier::DIM` in host production code (forms, pin edit, previews, viewer, pane viewer, controls, badge, toast, node-kind sentinel); forms take `&Theme` with `field`, `field_active`, `field_cursor` tokens whose defaults reproduce today.
6. One key list: pantry and docs read it; two-way drift test against docs/operations.md; alias tests per old key.
7. Docs: operations.md theme section, new ADR amending 0032, tuikit control page and architecture row, operator-review entries. Snapshot before/after, full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency on CSP-700 (aider removal, m-24) dropped at execution time: m-24 is a separate operator workstream and may be in progress elsewhere. The harness palette adapter is written with the current harness set, including aider's colour and the harness_aider alias, and m-24 removes those entries from the adapter when it lands. Net cost to m-24 is a small deletion.
<!-- SECTION:NOTES:END -->
