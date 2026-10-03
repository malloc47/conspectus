---
id: m-13
title: "Phase 8: Interactive TUI"
---

## Description

Source plan: `docs/implementation/phase-08-interactive-tui.md`.

Phase goal: add `conspectus tui`, a keyboard-first terminal UI for
searching, selecting, inspecting, and attaching/resuming the graph rows
already exposed by `conspectus table <ROWS>` and `conspectus node show`.

Dependency shape inside the phase:

```
CSP-160 ──→ CSP-160.01 ──→ CSP-161 ──→ CSP-162 ──→ CSP-163 ───┬─→ CSP-165 ─┐
                                                  ├─→ CSP-164   ┤          │
                                                  └─→ CSP-166   ┤          │
                                                                └─→ CSP-167 ┼─→ CSP-172
                                                                            │
                                            CSP-168 ─────────→ CSP-169 ─→ CSP-170 ───┤
                                                                        └─→ CSP-175 ─┤
                                                                            │
                                            CSP-171.01 ─────────────────────┤
                                            CSP-171.02 ─────────────────────┤
                                            CSP-171.03 ─────────────────────┘
```

`CSP-160`, `CSP-160.01`, `CSP-161`, and `CSP-162` are closed (v1 product
vision, v1-blocking decisions, the runtime/architecture ADR, and
the `conspectus tui` shell with terminal lifecycle are all in
place). `CSP-163` through `CSP-166` can be
implemented in parallel once the app shell exists. `CSP-168` through
`CSP-170`, `CSP-175`, and the `CSP-171*` enrichments depend on the same
UI shell but should remain isolated from pure browsing/rendering
work. `CSP-175` is post-v1 polish that does not block the release.
`CSP-176` is a post-v1 sessions-tree refinement layered onto
`CSP-163` and `CSP-174`; it does not block the v1 release either.

- **CSP-160** Lock v1 TUI product decisions (operator-journey core)

- **CSP-160.01** Settle remaining v1-blocking product questions

- **CSP-161** ADR: TUI runtime, app architecture, and dependency policy

- **CSP-162** Add `conspectus tui` CLI shell and terminal lifecycle

- **CSP-163** Build TUI row tree view-models for every table row-type

- **CSP-164** Build selected-node detail view-models

- **CSP-165** Implement selection, focus, navigation, and filtering state

- **CSP-166** Render the two-panel Ratatui UI

- **CSP-167** Add non-blocking graph refresh data adapter

- **CSP-168** Add mux live-preview capture adapter

- **CSP-169** Implement attach-to-existing-mux action

- **CSP-170** Implement resume un-muxed agent session into mux

- **CSP-171** Add PR, fork, and transcript/history detail enrichments

- **CSP-171.01** PR right-panel enrichment with async `gh` fetch + cache

- **CSP-171.02** Fork right-panel enrichment (lineage, context, children)

- **CSP-171.03** Un-muxed agent transcript preview

- **CSP-172** Document and verify the v1 TUI workflow

- **CSP-175** Inline mux-picker for ambiguous `LinkedToMux` candidates

- **CSP-176** Surface session `title` in the sessions row tree when it uniquely distinguishes siblings

- **CSP-177** Populate `AgentSessionNode.last_active_epoch` across harness adapters

- **CSP-178** Collapse duplicate repo group rows when a project appears across multiple checkout buckets in the TUI sessions row tree

- **CSP-180** Fill out the TUI empty/loading/error frame matrix

- **CSP-181** Same-line session preview switch

- **CSP-182** Header `updated Ns ago` freshness indicator

- **CSP-183** Expand TUI buffer-snapshot test coverage

- **CSP-185** Throttle and freshen mux pane-capture previews

- **CSP-186** Render ANSI color in tmux previews

- **CSP-187** Strengthen selected-row and focused-pane visual states

- **CSP-188** Compress project, path, and mux display labels

- **CSP-299** Show full session and mux IDs in the TUI

- **CSP-300** Expand linked entities from the TUI detail pane

### Detail Pane Graph Explorer Revamp

Runtime process observations made the current one-hop inline expansion
model too dense: repeated nested `Mux`, `Session`, and `Process`
section headers are hard to scan, and indentation is not enough to
preserve orientation. Per `docs/design.md`, the right panel should be a
focused node inspector plus relationship explorer. Core facts for the
selected node stay visually separate; upstream/downstream links render
as compact relationship rows; the selected relationship gets a compact
preview; graph depth is reached by drilldown with breadcrumbs rather
than recursive inline detail panes.

- **CSP-313** Model detail-pane relationship groups and previews

- **CSP-314** Replace inline expansion with relationship-group navigation

- **CSP-315** Render the focused inspector, relationship explorer, and preview layout

- **CSP-316** Add full-value inspection for long detail fields

- **CSP-317** Update docs and scenario coverage for detail graph navigation

- **CSP-318** First-class evidence inspector and link-promotion flow

- **CSP-318.01** Surface resolver explanations in the TUI detail explorer

- **CSP-319** TUI responsive-layout design and breakpoints

- **CSP-320** Expanded Node Detail toggle

- **CSP-321** Left-pane mirror sync (default)

- **CSP-322** Left-pane follow sync (opt-in view switching)

- **CSP-323** Distinguish symmetric relations in the detail explorer

- **CSP-324** Shorten breadcrumb hop labels and elide deep chains

- **CSP-325** Surface node kind as a first-class field in the detail pane

- **CSP-326** Enter-to-copy on Node-zone fields with a toast widget

- **CSP-327** Flip the Upstream / Downstream header layout so zone labels anchor to the right

- **CSP-328** Hide edge meta (`provenance · confidence · state`) from link rows by default with an opt-in toggle

- **CSP-196** Auto-broaden TUI scan roots to the cwd's "code dir" ancestor when neither CLI nor config specifies one

- **CSP-267** Descend into scan roots when looking for atelier workspaces (companion to `CSP-196`)

- **CSP-189** Default-expand and mark the launch-context project without filtering the world

- **CSP-190** Make the status bar contextual to the selected row

- **CSP-353** Make Enter trigger the selected row's default action

- **CSP-191** Add sessions-tree density modes

- **CSP-192** Crop and annotate tmux previews for recognition

- **CSP-194** Round-trip attach: return to the TUI after the operator detaches from the mux client

- **CSP-195** Auto-scroll the left tree to keep the selected row visible

- **CSP-193** Add visible search/filter workflow for large session worlds

- **CSP-184** Move TUI discovery onto a background thread with timer-driven refresh

- **CSP-179** Align `conspectus table sessions` columns with the TUI sessions row tree once view-models converge

### Filter, View Switching, And Per-View State (F8-*)

ADR 0031 settled the design for layered structured filters + `/`
fuzzy search, per-view filter/grouping/selection/expanded state
(sort stays global), per-view grouping enums, a shared `RowFilter`
predicate driving both CLI `table` and the TUI, and a discoverable
Controls overlay fronting the capability with accelerator keys layered
on top. Stories below carve that ADR into implementable slices.

Dependency shape inside the workstream:

```
ADR 0031 ─→ CSP-250 ─→ CSP-258 ─→ CSP-259
            │
            ├─→ CSP-251 ─→ CSP-252 ─┐
            │                       │
            ├─→ CSP-255 ─┐          │
            │            ↓          ↓
            ├─→ CSP-253 ─→ CSP-254 ─→ CSP-260
            │            ↓
            └─→ CSP-256 ─→ CSP-261
                CSP-257 (independent loader work)
```

`CSP-250`/`CSP-251`/`CSP-255`/`CSP-257` can land in parallel after the
ADR. `CSP-258` and `CSP-259` extend the CLI surface and table
projections respectively. `CSP-253` (controls overlay) and the
status-bar / empty-frame stories converge on `CSP-254` for the
keybinding surface; `CSP-260` documents the final keymap once it
settles.

- **CSP-250** Define `RowFilter` predicate + dimension types in a crate-public module

- **CSP-251** Per-view grouping enums for the four pending views

- **CSP-252** Per-view state retention

- **CSP-253** Controls overlay (modal)

- **CSP-254** Accelerator keybindings + view-switching plumbing

- **CSP-255** Multi-select list widget

- **CSP-256** Status-bar filter chips + counts-with-totals

- **CSP-257** `[tui.views.<name>]` config schema + legacy alias

- **CSP-258** CLI flag parity: shared `FilterArgs` + per-view grouping

- **CSP-259** `conspectus table <ROWS>` consumes `RowFilter`

- **CSP-260** Help-overlay docs

- **CSP-261** Filtered-zero empty frame

- **CSP-423** Persist last-active view across TUI restarts

- **CSP-424** Spike: evaluate `tui-pantry` as a widget-iteration harness

- **CSP-268** Detect session live status (running / waiting / idle / error) and surface it as a row glyph and per-status header chip

- **CSP-269** Ship preset theme variants on top of ADR 0032

- **CSP-270** Add sessions-tree density modes — folds `CSP-191` into the theme-aware renderer landed by the styling overhaul
