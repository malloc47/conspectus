# Graph Visualization

Conspectus exports the resolved graph in three formats:

| Format | Use case |
| --- | --- |
| `json` | Machine-readable canonical form. Default. Pipe into `jq`, diff against fixtures, feed into other tools. |
| `dot` | Static inspection. Pipe through Graphviz (`dot -Tsvg`, `dot -Tpng`) for design docs, PRs, code reviews. |
| `html` | Interactive exploration. Single self-contained file with Cytoscape-backed pan/zoom, filter panel, inspector, search, and focus/depth/direction navigation. |

DOT and HTML render the same resolved graph as `--format json`. The shaping
decisions are settled by [ADR 0050](adr/0050-graph-visualization-exports.md);
this guide is the operator-facing reference.

## Commands

```sh
# Default JSON (full evidence-preserving graph)
conspectus graph --format json [--scan-root PATH]...

# Graphviz DOT
conspectus graph --format dot [--scan-root PATH]...
    [--candidates {include,exclude}]
    [--diagnostic-nodes {include,exclude}]

# Self-contained HTML explorer
conspectus graph --format html [--scan-root PATH]...
    [--candidates {include,exclude}]
    [--diagnostic-nodes {include,exclude}]
```

Debug builds also expose the named replay scenarios from `TEST-006`:

```sh
conspectus dev scenario graph --format {json,dot,html} <scenario-name>
```

See [`docs/dev-scenarios.md`](dev-scenarios.md) for the available scenario
names.

### Flags

- `--candidates include` (default) keeps every `GraphLink` candidate edge in
  the export, including ones the resolver did not pick. `--candidates exclude`
  drops them so only resolver-preferred relationships render.
- `--diagnostic-nodes include` (default) keeps `RuntimeProcess` nodes and
  every edge touching them. `--diagnostic-nodes exclude` drops them — useful
  when you want the operator view without the process-attribution layer.
- The HTML chrome has its own runtime filter panel that layers on top of
  what the renderer emitted; see "HTML chrome" below.

## DOT

The DOT renderer produces a `digraph` keyed on `NodeKind` (shape and fill),
`RelationKind` (arrowhead category), and `Provenance` (penwidth and color
tint). Resolver-preferred candidates are solid and carry a `★` suffix;
losing candidates dashed; ignored/overridden dashed-red with a tooltip
naming the override; unresolved endpoints render as dashed-circle stubs.
Nodes group into per-`NodeKind` `subgraph cluster_*` blocks.

Emission is deterministic (sorted by `NodeId` and edge key), so the same
graph produces the same DOT every time.

### Recipes

```sh
# Quick SVG of the current working directory's graph
conspectus graph --format dot | dot -Tsvg > graph.svg

# PNG for an issue attachment
conspectus graph --format dot | dot -Tpng > graph.png

# Resolver-only view (no candidate-evidence noise)
conspectus graph --format dot --candidates exclude | dot -Tsvg > resolved.svg

# Inspect a named scenario without re-creating local state
conspectus dev scenario graph --format dot ambiguous-mux | dot -Tsvg > ambiguous.svg

# High-resolution PDF for design docs
conspectus graph --format dot | dot -Tpdf > graph.pdf
```

If a graph is too dense for Graphviz to lay out cleanly, try:

- `dot -Kfdp` (force-directed) or `dot -Ksfdp` (multiscale) instead of the
  default hierarchical layout
- `--candidates exclude --diagnostic-nodes exclude` to drop the diagnostic
  layers
- `--scan-root` to narrow the discovery surface

## HTML

The HTML renderer produces a **single self-contained `.html` file**
(typically 750-800 KB; the page inlines the vendored Cytoscape.js bundle,
its `fcose` layout extension, and the Conspectus chrome). No network
requests at view time; the file is safe to attach to an issue, drop into a
shared folder, or open from an air-gapped machine.

```sh
# Generate and open
conspectus graph --format html > graph.html && xdg-open graph.html

# macOS
conspectus graph --format html > graph.html && open graph.html

# For a named scenario
conspectus dev scenario graph --format html workspace-pr > /tmp/ws.html
```

### HTML chrome

The page is a three-column layout: filter panel left, graph canvas center,
inspector / legend right. A header search input sits top-right; a focus
toolbar appears above the canvas when you focus a node.

#### Filter panel (left)

- **Presets**: "Collapsed view" approximates what `conspectus table sessions`
  would show (hides candidates, `RuntimeProcess` nodes, unresolved stubs,
  and ignored/overridden edges). "Reset" restores the default everything-on
  view.
- **View toggles**: show candidate links / show RuntimeProcess / show
  unresolved endpoints / show ignored & overridden.
- **Node kinds**: per-`NodeKind` checklist with counts. Unchecking a kind
  hides every node of that kind and (implicitly) every edge incident to it.
- **Relations**: per-`RelationKind` checklist with counts. Unchecking a
  relation hides every edge of that relation but keeps the endpoint nodes.

#### Inspector (right)

Replaces the legend when a node is selected. Shows the kind, primary +
secondary labels, the full `NodeId`, every attribute (flattened to dotted
paths), and grouped incoming/outgoing edges. Each edge row carries the
neighbor's kind swatch + label, the provenance, the confidence, a `★` if
the resolver picked it, and a state badge for ignored/overridden links.
Clicking a neighbor row navigates the inspector and focuses on that node.

A **Focus** button in the inspector header pushes the selected node into
the navigation stack (see below).

#### Search (header)

Free-text substring match on `label_primary`, `label_secondary`, `kind`,
and `id` (case-insensitive). Non-matching nodes and their incident edges
are dimmed rather than hidden, so you can see the graph context around the
matches. The header shows the match count. Clear the input to remove the
dim.

#### Focus navigation (top of canvas)

The navigation toolbar appears when a node is focused. Restricts the
visible graph to nodes within a configurable depth of the focused node.

- **Depth**: `1` / `2` / `3` / `All` chip group. Shortcuts: `[` decreases
  depth, `]` increases.
- **Direction**: `Both` / `↑ Upstream` / `↓ Downstream` chip group.
  Upstream follows incoming edges only; Downstream follows outgoing only;
  Both treats the graph as undirected for traversal purposes.
- **Breadcrumb**: every focus push is recorded. Click any historic crumb
  to jump back, or use the `← Back` button (shortcut: `Backspace`) to pop
  one step.
- **Clear focus**: restores the full graph (shortcut: `Esc`).

Keyboard shortcuts are suppressed while the search input has focus, so
typing `[` or `]` into the search box does what you expect.

### Debugging recipes

**"Why did the resolver pick mux X over mux Y for this session?"**

1. Open the HTML for the affected scope.
2. Find the agent session in the graph (use search if needed).
3. Click it to populate the inspector. The `Outgoing` section groups
   candidate `linked_to_mux` edges; the resolver's pick has a `★`, the
   others don't. Each edge row shows the provenance + confidence that fed
   into the decision.
4. Hover the dashed losing candidate to see its provenance (e.g.
   `convention`, `cached`) and confirm it lost to the solid `★` edge.

**"Show me everything reachable from this fork."**

1. Click the fork node, then click "Focus" in the inspector header.
2. The graph collapses to the fork's depth-2 neighborhood. Step depth up
   (`]`) to widen.
3. To see only what the fork created/affected, switch direction to
   `↓ Downstream`.

**"Approximate the CLI table view."**

1. Click "Collapsed view" in the filter panel.
2. The graph drops candidates, `RuntimeProcess` nodes, unresolved
   endpoints, and ignored/overridden edges. What's left is the resolver's
   answer — the same data the CLI tables and TUI present.

**"What's this `RuntimeProcess` evidence for?"**

1. Make sure "Show RuntimeProcess" is on in the filter panel.
2. Click a process node. The inspector shows the process's command, cwd,
   pid, role, and any `process_identifies_session` /
   `process_candidates_session` edges out to agent sessions.
3. Click the linked session via the inspector's edge row to navigate.

## Coverage and limits

- DOT and HTML render the same resolved snapshot as `--format json`. They
  do not query, mutate, or persist anything.
- Both formats respect `--candidates` and `--diagnostic-nodes`; the HTML
  chrome layers additional runtime filtering on top.
- HTML output is offline-safe by construction. Library updates are
  deliberate operator actions (see `src/output/html/assets/VERSIONS`).
- Visual encoding is provider-neutral: shape/color are keyed on
  `NodeKind`, arrowhead on `RelationKind`, weight/opacity on `Provenance`.
  Provider identity (atelier, tmux, github, harness key, mux backend) is
  carried in attributes and the HTML inspector panel; per-provider
  styling is deferred until concrete use cases appear (ADR 0050
  decision 3).
- A live server-hosted HTML view backed by `conspectus serve` is an
  explicit follow-on (ADR 0050 Open Questions); the v1 export is
  one-shot.
- Layout tuning beyond the current fcose defaults is tracked separately;
  open the rendered HTML and use the chrome controls to compensate when
  the default layout is unkind to a particular graph.

See [ADR 0050](adr/0050-graph-visualization-exports.md) for the locked
design decisions, including the Coupling Boundary that keeps Cytoscape a
swappable implementation detail.
