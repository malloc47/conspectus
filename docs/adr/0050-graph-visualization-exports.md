# ADR 0050: Graph Visualization Exports

## Status

Accepted

## Context

The Conspectus graph is opaque outside the existing JSON and tabular
projections. When the resolver makes a surprising choice, when a TUI row
renders unexpectedly, or when a named replay scenario gains new
candidate/resolved structure, there is no compact way to see the graph
shape directly. The backlog tracks this as the **GV** workstream:
`GV-001` (this ADR), `GV-002` (`conspectus graph --format dot`),
`GV-003` (`conspectus graph --format html`), and `GV-004` (workflow
docs).

The blockers are now cleared. `H-MUXPROC-FU-006` shipped node-detail
process diagnostics and the `process-cardinality` /
`codex-fd-current` scenarios; `TEST-006` exposed named replay
scenarios to the CLI and TUI through `conspectus dev scenario …`. The
graph model is stable enough to visualize: 8 node kinds (`Repo`,
`Checkout`, `Branch`, `Workspace`, `AgentSession`, `MuxSession`,
`Fork`, `ForgePr`, plus the candidate `RuntimeProcess` from ADR
0047), 20 `RelationKind` variants, the candidate/resolved split from
ADR 0002, and provenance/confidence carried on every link.

The current `conspectus graph` command at `src/cli.rs:1105` accepts
`--format json` and a `--scan-root` list. `OutputFormat` is the
single-variant enum the new formats plug into.

Several decisions interact and need to be settled together so
`GV-002` and `GV-003` are not blocked relitigating them:

1. **Output formats and what each is for.** DOT is for piping through
   Graphviz (`dot -Tsvg`, `dot -Tpng`) for static inspection, code
   review attachments, and copy-paste into design docs. HTML is for
   interactive exploration: pan/zoom, search/filter, node selection,
   neighbor highlighting, and an inspector panel.
2. **How the HTML loads its graph library.** Vendored asset on disk,
   external CDN at view time, or single-file self-contained bundle.
   ADR 0016 (distribution) discourages surprise runtime fetches, and
   the backlog scope explicitly calls out "keep the exported file
   usable offline if the ADR selects vendoring or self-contained
   output."
3. **Provider-neutrality of the visual encoding.** Per CLAUDE.md
   guardrails the core graph stays provider-neutral. The visual
   encoding has to be settled along that line: shape/color encodes
   model concepts (`NodeKind`, `RelationKind`, `Provenance`) and
   provider identity is carried in attributes/tooltips rather than
   bespoke shapes per provider.
4. **Candidate vs. resolved relationships.** ADR 0002 separates
   `GraphLink` candidates from typed resolved relationships. The
   visualizer has to make both visible, since "why did the resolver
   pick that?" is the primary debugging use case.
5. **Runtime-process visibility.** Per ADR 0047, default human
   projections may hide `RuntimeProcess` nodes, but graph JSON,
   SQLite, node detail, and visualization should preserve them.
6. **Unresolved-endpoint evidence.** Per ADR 0005 and ADR 0018,
   session-lineage candidates can have unresolved endpoints. They are
   real edges with one missing terminal, not edges to be silently
   dropped.
7. **Large-graph degradation.** Force-directed layouts degrade
   visually long before they degrade computationally. The ADR needs
   to set a starting position rather than leaving each renderer to
   improvise.
8. **Determinism.** DOT and HTML snapshot/fixture tests are part of
   the backlog scope. They are unstable without a committed ordering
   rule.
9. **Theme sharing with the TUI.** ADR 0032 ships a `[tui.theme]`
   table with harness colors but no per-`NodeKind` colors. The
   visualizer needs per-`NodeKind` colors. Either the visualizer
   defines its own palette or the theme schema grows to cover both
   surfaces.
10. **Navigation UX in the HTML.** The interactive use cases
    ("show me the graph from this node," "show me only upstream /
    downstream," "show me N-depth from this node") need either a
    pre-built UI or a bespoke wrapper around the rendering library.

## Decision

### 1. Two formats, complementary roles

`conspectus graph --format {json,dot,html}`. JSON stays the canonical
machine-readable form (ADR 0002 / ADR 0044). DOT is the static
inspection format intended to be piped through Graphviz. HTML is the
interactive explorer. DOT and HTML are derived projections of the same
resolved `GraphSnapshot`; neither replaces JSON.

Both formats accept `--scan-root` like the existing JSON branch, and
both render named replay scenarios through
`conspectus dev scenario graph --format {dot,html}` once `TEST-006`'s
scenario plumbing is extended (already present for JSON; the new
formats follow the same path through `ScenarioWorld::snapshot`).

### 2. HTML delivery: single-file self-contained bundle

The HTML output is a single `.html` file with the chosen JavaScript
library inlined via `include_str!` at build time. No external network
fetches; no separate asset directory; no CDN. The file is safe to
attach to an issue, drop into a shared folder, or open from an air-
gapped machine.

This commitment forces the chosen library to be (a) MIT/BSD/Apache or
similarly permissive, (b) small enough that inlining is reasonable,
and (c) a single bundled JS file with no runtime asset loading.

The library is **Cytoscape.js** (MIT). Rationale:

- Single-file UMD build (~350 KB minified), inlines cleanly via
  `include_str!`.
- Native support for compound nodes (used here for kind-based
  clustering) and dashed edge styling (used for unresolved
  endpoints).
- Stable, widely deployed, well-documented API surface.
- Permissively licensed and ships extension points (`cytoscape-fcose`
  for force-directed layout, `cytoscape-dagre` for hierarchical
  layout) that can be added later without changing the export shape
  if needed.

Alternatives considered:

- **vis-network** (Apache 2.0). Comparable feature set, simpler
  defaults, but the compound-node + edge-style story is less flexible
  and the bundle is larger.
- **D3-force / raw D3**. Maximum control but no batteries; the
  navigation UX would need to be written from scratch on top of raw
  selections. Higher implementation cost than Cytoscape.
- **Sigma.js**. Strong for very large graphs (WebGL), but the
  Conspectus graphs in scope are small to medium; the optimization
  is not yet worth the heavier integration story.
- **Graphviz WASM in the browser**. Would let the HTML export reuse
  the DOT renderer directly. Rejected: the WASM blob is multi-MB,
  layout is static after render, and there is no node-selection /
  inspector affordance.

### 3. Provider-neutral visual encoding (v1 default)

The v1 visual encoding is keyed on the model, not on providers:

- **Node color and shape** are determined by `NodeKind`.
- **Edge style** is determined by `RelationKind` (and whether the
  edge is candidate or resolved; see decision 4).
- **Edge weight / opacity** reflects `Provenance` precedence (declared
  > strong-discovered > discovered > convention > cached).
- **Provider identity** (`atelier`, `tmux`, `github`, the harness
  key, the mux backend name, …) is carried as tooltip / inspector
  metadata in v1, rather than as a bespoke node shape or color.

Adding a new provider should not require changes to the visualizer
encoding by default. Adding a new `NodeKind` or `RelationKind` does
require a palette update; that is consistent with the model-first
guardrail.

Provider-specific colors, glyphs, or inspector fields are not
prohibited — they are deferred. When a concrete use case appears
(e.g. one provider's metadata is materially different in inspection
flows, or distinguishing two harnesses by color materially helps
users), a follow-up can add provider-keyed overrides on top of the
`NodeKind`-keyed base palette without re-litigating this decision.
The expectation is that such overrides ride on the shared `[theme]`
table from decision 9 rather than introducing a parallel
provider-styling surface.

### 4. Candidate vs. resolved as toggleable views

The HTML explorer treats "candidate graph" and "resolved graph" as two
views over the same underlying export, switchable from a top-level
toggle in the UI:

- **Resolved view (default).** Renders only resolved relationships.
  This is the projection the TUI and tables consume. Default-on so
  the visualizer matches what users see in the rest of the product.
- **Candidate view.** Renders all `GraphLink` candidates. Resolver-
  preferred candidates carry a `★` marker and are drawn with higher
  contrast; non-preferred candidates are drawn dimmer. Conflicted or
  ignored candidates are distinct again.

The DOT export emits both layers in one graph (no toggle in DOT,
since DOT is a static format). Candidate edges are drawn with a
distinct style so a reviewer can see the resolver's decision context
without running a second command. A `--candidates {include,exclude}`
CLI flag selects whether the DOT export collapses to resolved-only;
the default is `include`.

### 5. Runtime-process and other "diagnostic" nodes filterable

`RuntimeProcess` nodes (ADR 0047) are included in both DOT and HTML
exports by default, consistent with the ADR 0047 rule that
visualization preserves them. The HTML explorer ships a filter panel
with at least these toggles:

- show / hide `RuntimeProcess` nodes
- show / hide unresolved-endpoint stubs
- show / hide ignored / overridden candidate links
- a `NodeKind` checklist for arbitrary kind-level filtering
- a `RelationKind` checklist for arbitrary relation-level filtering
- a free-text search across node label and id

A "collapsed view" preset turns the diagnostic-heavy toggles off in
one click so the HTML graph approximates what the CLI/TUI would show.
This is in addition to (not a replacement for) per-toggle control.

DOT exports gain a `--diagnostic-nodes {include,exclude}` flag
(default `include`) for the same purpose.

### 6. Unresolved endpoints render as dashed stubs

Per ADR 0005 / ADR 0018, session-lineage and similar candidate links
can have one endpoint unresolved. Visualization renders these as
edges from the known endpoint to a small dashed-terminator stub node
labeled with the unresolved identifier. The stub is styled distinctly
from real nodes and is suppressible via the unresolved-endpoint
filter from decision 5.

### 7. Large-graph degradation

DOT exports use `subgraph cluster_*` for kind-based clustering by
default. The clusters are visual only and do not change link
semantics. This is cheap, deterministic, and produces readable output
under Graphviz's default layouts.

HTML exports start with **no automatic clustering**. The first
implementation runs Cytoscape's force-directed layout
(`fcose` or the built-in `cose`) at full fidelity and exposes
clustering as an opt-in toggle. The intent is to learn how the
default layout actually behaves on real graphs before committing to a
threshold. If/when a threshold becomes obviously necessary, a follow-
up sets it; the ADR does not prejudice the number.

This is intentional: under-clustering is reversible (the user opts
in), but baking in a low threshold up front hides the layout
characteristics we want to observe.

### 8. Deterministic emission

Both DOT and HTML emit nodes and edges in a stable order:

- Nodes sorted by `(NodeKind, NodeId)`.
- Edges sorted by `(source NodeId, RelationKind, target NodeId,
  Provenance)`.
- Any embedded timestamps are normalized to ISO-8601 UTC and may be
  redacted under a `--fixed-time` flag used by snapshot tests.
- Floating-point confidence values are rendered with a fixed-width
  format so trivial precision drift does not destabilize snapshots.

This is a non-negotiable contract that snapshot tests in `GV-002` /
`GV-003` rely on.

### 9. Top-level `[theme]` with surface-specific overrides

ADR 0032's `[tui.theme]` becomes a special case of a broader scheme:

```toml
[theme]
# Shared across all rendered surfaces. Includes per-NodeKind colors
# (new — these did not exist for the TUI because rows are harness-
# keyed, not kind-keyed), per-Provenance modifiers, and per-relation
# accent hints where applicable.

[tui.theme]
# Existing surface — overrides the shared [theme] for TUI-specific
# concerns (harness colors, recency buckets, etc., as ADR 0032).

[html.theme]
# New surface — overrides the shared [theme] for HTML-specific
# concerns (background, edge widths in pixels, font stack).
```

Migration of the existing `[tui.theme]` keys is staged: the existing
keys keep working unchanged. The shared `[theme]` table is additive
and starts with per-`NodeKind` colors plus per-`Provenance` styling
hints — the values the visualizer needs and that the TUI does not
yet expose. Individual agent harnesses are not given per-harness
visualization colors in v1; harness-level color stays a TUI concern
until the visualization shows a need for it.

A separate follow-up ADR can collapse `[tui.theme]` keys into
`[theme]` if usage patterns make that worthwhile, but the v1
visualizer does not require that work to be done first.

### 10. Drop-in navigation UI on top of Cytoscape, not a bespoke
explorer

The HTML output ships with a thin layer of bespoke UI chrome (filter
panel from decision 5, candidate/resolved toggle from decision 4,
inspector panel for selected node) wrapping Cytoscape's built-in
interactions. For graph-navigation primitives ("focus on this node,"
"show neighbors only," "show N-depth from this node," "show only
upstream / downstream"), Conspectus implements them directly against
the Cytoscape API rather than adopting a heavier turn-key explorer
library.

This is a deliberate trade-off:

- Mature drop-in Cytoscape explorer UIs exist (e.g.
  `cytoscape.js-navigator`, the older NDEx Cytoscape Web viewer, and
  several bioinformatics-domain explorers), but they are scoped to
  specific domains (biology pathway browsing, social networks) and
  drag in opinionated chrome and assumptions that fight a provider-
  neutral developer-tools graph.
- The navigation operations we actually need are short: BFS from a
  selected node up to `--depth N`, direction-restricted traversal
  (incoming-only / outgoing-only), and "hide everything not in the
  current selection set." These are tens of lines on top of the
  Cytoscape collection API and stay aligned with Conspectus's own
  graph terminology.
- The HTML bundle stays smaller and the dependency footprint stays
  auditable. Inlining a single library is sustainable; inlining a
  library plus an explorer UI plus its theming assets is not.

The bespoke chrome lives in a small JS module checked in alongside
the Rust renderer and inlined into the export the same way the
Cytoscape bundle is. It is feature-targeted, not a framework.

#### Coupling Boundary

Cytoscape is chosen as the v1 rendering library, but it is treated as
an implementation detail of a single JS module rather than as the
foundation the rest of the visualizer is built on. The intent is that
swapping to a different library (e.g. AntV G6 with Graphin, if the
project later decides a React-based explorer chrome is warranted)
rewrites that one module without touching the Rust renderer, the
payload contract, or the chrome.

Concretely:

- **Rust emits a library-neutral JSON payload.** The HTML export
  extends the existing `render_graph_json` shape (ADR 0002 / ADR
  0044) with whatever additional fields the visualizer needs
  (per-`NodeKind` display hints, candidate/resolved partitioning,
  unresolved-endpoint markers). It does not emit Cytoscape's
  `{data, position, classes}` element format. Any library-specific
  transformation happens in JS.
- **A `GraphDriver` interface fronts every library call.** The chrome
  (filter panel, candidate/resolved toggle, inspector, search,
  breadcrumb, keyboard handlers) calls the driver through a small
  named interface (`load(payload)`, `focusNode(id)`,
  `applyFilter(predicate)`, `getNeighborhood(id, depth, direction)`,
  `setStylesheet(neutralStyle)`, …). No chrome module reaches into
  the underlying `cy` instance directly.
- **Graph-traversal primitives are implemented over the neutral
  payload, not over the library's collection API.** BFS, depth-N
  expansion, and direction-restricted traversal (upstream-only /
  downstream-only) operate on the JSON node/edge arrays. This costs
  marginally more code than calling `cy.collection().bfs()` but
  keeps the primitives portable.
- **Stylesheet is expressed in a neutral declarative form.** The
  visualizer defines styling in terms of `NodeKind` /
  `RelationKind` / `Provenance` (per the shared `[theme]` from
  decision 9). The driver translates that to Cytoscape's stylesheet
  at load time. A future driver translates the same neutral form to
  its own library's idiom.

This boundary is not free — graph-traversal-over-payload is more
verbose than collection chaining, and the stylesheet translator is
an extra layer — but the cost is small and the asymmetry favors it:
swapping rendering libraries with the boundary in place is a
contained ~1-2 week rewrite of the driver module; without the
boundary it is a full rewrite of the entire JS surface. The same
boundary also makes the live-server view (see Open Questions) easier
to land, because the server can reuse the chrome verbatim and only
needs the driver to be wired against a streaming payload source.

## Consequences

- `conspectus graph` grows two formats and the `OutputFormat` enum at
  `src/cli.rs:3219` gains `Dot` and `Html` variants. The single
  scenario-render path through `ScenarioWorld::snapshot` is reused.
- A new `conspectus::output` submodule (e.g. `output::dot`,
  `output::html`) carries the renderers. They consume
  `&GraphSnapshot` and return `String` like the existing
  `render_graph_json`.
- A vendored copy of Cytoscape.js (UMD build) and the bespoke
  navigation/inspector JS module live under `src/output/html/assets/`
  and are pulled in via `include_str!`. License headers ride along.
  Updating the vendored copy is a deliberate operator action; the
  CHANGELOG entry has to identify the version bump.
- The shared `[theme]` table is new config surface and needs a
  parallel diagnostic policy (soft-fail to defaults, per ADR 0032).
- Snapshot tests in `GV-002` / `GV-003` lock in the deterministic-
  ordering contract from decision 8. Future renderer changes that
  perturb that order are breaking changes to the test fixtures and
  have to be intentional.
- The candidate-vs-resolved toggle in HTML and the inclusive default
  in DOT mean the visualizer can be used as a resolver debugger
  without rerunning discovery; that is a positive consequence for the
  user's stated motivation ("understand why the TUI is rendering as
  it does").
- Provider-neutral encoding means a new provider gains visualization
  coverage automatically through the existing `NodeKind` /
  `RelationKind` mapping. No visualization changes are needed when
  adding, e.g., a new mux backend.

## Alternatives Considered

- **CDN-hosted JS library.** Rejected: violates the offline-usable
  guarantee and conflicts with ADR 0016's distribution stance.
- **Vendored asset alongside HTML.** Rejected: forces users to ship
  two files together; one of the most common workflows is "attach the
  HTML to an issue."
- **DOT-only, no HTML.** Rejected: the user's stated need ("visually
  inspect the graph to understand TUI behavior") wants interactive
  filtering and selection, not a static PNG.
- **HTML-only, no DOT.** Rejected: DOT is the most natural format to
  paste into design docs, run through `dot -Tsvg`, and diff in a
  pull request.
- **Provider-keyed visual encoding as the v1 base.** Deferred rather
  than rejected. CLAUDE.md's provider-neutrality guardrail keeps the
  core graph shape model-keyed, and the v1 visualizer follows that:
  the base palette is `NodeKind` / `RelationKind` / `Provenance`,
  and provider identity lives in attributes and tooltips. Provider-
  specific overrides layered on top of the base palette are an
  acceptable later addition once concrete use cases appear; see
  decision 3 for the path.
- **Single combined view (no candidate/resolved toggle).** Rejected:
  the resulting render is visually noisy and conflates two different
  questions ("what does the resolver believe?" vs. "what evidence did
  it see?"). The toggle keeps each view legible.
- **Auto-cluster aggressively from the start in HTML.** Rejected: we
  do not yet know how unclustered layouts perform on real graphs.
  Starting permissive lets us learn before committing.
- **Drop in a turn-key Cytoscape explorer UI.** Rejected: see
  decision 10. The domain mismatch and bundle-size cost outweigh the
  saved implementation effort for the small set of navigation
  operations we actually need.

## Open Questions

### Live view served by `conspectus serve`

The static HTML export is the v1. A natural follow-on is a
**live HTML view** served directly by the continuous server (ADR
0038), where the page connects back to the server (most likely over
the existing Unix socket via a small HTTP shim, or over a localhost
loopback) and re-renders the graph as discovery refreshes land. This
is deliberately out of scope for `GV-003`.

A follow-up ADR should resolve, at minimum:

- Transport: localhost HTTP loopback vs. extending the Unix-socket
  protocol vs. SSE / WebSockets on top of either.
- Authn/authz model: localhost-only, token-in-URL, or something
  stricter.
- Update semantics: full re-render per refresh, diff push per
  provider slice (which the persistence layer already supports per
  ADR 0037), or client-driven polling.
- Reuse of the static-export HTML chrome: the live view should reuse
  the bespoke navigation/inspector module from decision 10 rather
  than reimplementing it.

This pathway is preserved by keeping the renderer's payload shape
(JSON embedded into the static HTML) identical to what a live view
would receive over the wire.

### Theme schema migration

Whether and when to collapse `[tui.theme]` keys into the shared
`[theme]` table is left to a follow-up. v1 keeps both tables; a later
ADR can consolidate once `[html.theme]` and any future surface
themes have shown which keys are actually shared in practice.

### Cytoscape layout choice

The first implementation picks one Cytoscape layout (most likely
`fcose`). Whether to expose layout selection as an HTML control,
make it a config key, or leave it hard-coded is deferred until the
default has been used against real graphs.
