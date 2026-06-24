# TUI Comparison Notes

These notes capture a design research pass over popular terminal UI
applications, with recommendations for future Conspectus TUI polish. They are
intended as reference material for backlog planning, not as accepted design
decisions.

## Reference Applications

- **bottom**: Rust/Ratatui system monitor with configurable layouts, themes,
  search, expansion, and compact graph widgets. Useful reference for dense
  metrics, fixed-width status columns, and layout customization.
- **btop**: visually distinctive system dashboard with strong use of color,
  gauges, graph-like widgets, and compact numeric presentation. Useful as a
  reference for flair, but Conspectus should stay more semantic and less
  decorative.
- **k9s**: Kubernetes operations TUI with fast resource switching, filters,
  skins, plugins, and problem-focused workflows. Useful reference for jumping
  directly to actionable state.
- **lazygit**: Git TUI with a compact commit graph, colored author/branch
  context, side-by-side panes, and command-driven workflows. Useful reference
  for lineage and PR/branch adjacency.
- **gitui**: Rust Git TUI emphasizing speed, low-latency navigation, compact
  panels, and keyboard-first workflows.
- **Harlequin**: SQL IDE TUI with catalog navigation, query/editor areas, and
  result/detail panes. Useful reference for multi-zone inspector layouts.
- **Posting**: API client TUI with jump mode, command palette, contextual help,
  compact mode, themes, keymaps, and tree-sitter highlighting. Useful reference
  for discoverability and modal command workflows.

## Current Conspectus Snapshot Observations

A showcase snapshot at `140x38` already shows a strong foundation:

- Dense header line with view name, freshness, visible counts, harness chips,
  and mux-state chips.
- Two-pane layout with a row tree on the left and selected-node details,
  related links, and preview on the right.
- Semantic node glyphs and colors for workspace/repo/session identity.
- Mux state vocabulary via `◉`, `◐`, and `◯`.
- Section dividers and validated relationship counts in the detail pane.
- Focus-aware status/footer text with common navigation commands.

The largest opportunity is not simply adding more color. The TUI should make
the graph's sparse state, evidence quality, conflicts, and next actions visible
at a glance.

## Ideas To Adopt

### Attention Mode

Add a persistent attention chip, lane, or view that jumps through actionable
graph problems:

- ambiguous mux attribution
- stale or unbound pins
- conflicting `GraphLink` candidates
- unresolved evidence
- PR/branch drift
- missing checkout/workspace links

This borrows from operational TUIs such as k9s, where users can move quickly
from global state to the objects needing action.

### Lineage Mini-Graph

Borrow lazygit's commit-graph idiom for Conspectus fork and session lineage. A
narrow fixed-width column using glyphs such as `│`, `├`, `╰`, and colored node
markers could make parent/child relationships visible in the main row tree
without opening the detail pane.

This is especially relevant for:

- fork provenance
- intra-harness session lineage
- parent/child agent sessions
- branch or checkout families

### Inspector Tabs

Evolve the right pane into explicit inspector modes:

- `Summary`
- `Links`
- `Evidence`
- `Preview`
- `Actions`

The current detail pane is useful, but tabs would reduce vertical sprawl and
make resolver evidence easier to explore without crowding the default summary.
Harlequin and Posting are good references for this style of multi-zone
inspection.

### Evidence Chips

Render graph provenance and resolver state as compact chips wherever links
appear:

- `declared`
- `discovered`
- `convention`
- `cached`
- confidence level
- provider/source
- conflict or alternate state

This directly supports Conspectus's data-model-first design: the evidence layer
becomes visible in the interface rather than hidden behind detail text.

### Density Modes

Provide row-density modes:

- `compact`: maximum sessions per screen, minimal previews, terse columns
- `normal`: current balanced layout
- `expanded`: multi-line previews, richer diagnostics, more edge metadata

Posting's compact mode and bottom's configurable layouts show how density can
be a first-class interaction rather than a fixed design choice.

### Focus-Sensitive Footer

Keep the footer compact, but make it more context-specific:

- left pane focused: navigation, grouping, filtering, expansion
- right pane focused: tab switching, evidence toggles, preview actions
- modal open: only modal-specific actions

This would improve discoverability as the keymap grows.

### Theme Presets

The TUI already has `[tui.theme]` plumbing. Add curated presets instead of
only exposing low-level color keys:

- default dark
- high contrast
- light
- subdued operator palette

Presets should stay semantic and restraint-oriented. Conspectus benefits more
from consistent state colors than from purely decorative palettes.

### Activity Micro-Visuals

Use small fixed-width activity indicators for scan-heavy columns:

- recency bucket
- transcript activity
- mux liveness
- PR freshness
- pin binding state

These should be stable-width so hover/focus/refresh states do not shift the row
layout.

## Design Fit For Conspectus

Conspectus is not a generic dashboard and should avoid adopting visual flair
that obscures graph semantics. The best borrowed ideas are those that improve:

- sparse graph scanning
- provenance and evidence comprehension
- conflict discovery
- session/fork lineage recognition
- keyboard-first navigation
- right-pane inspection depth

Future visual changes should continue to use snapshot validation via
`conspectus tui --snapshot` per ADR 0067.
