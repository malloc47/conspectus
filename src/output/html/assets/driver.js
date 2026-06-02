// driver.js — the only Cytoscape-aware layer (ADR 0050 decision 10
// Coupling Boundary). Chrome modules call into the GraphDriver
// interface defined here and never reach into the underlying `cy`
// instance directly.
//
// Translates the library-neutral payload emitted by the Rust
// renderer into Cytoscape elements + stylesheet at load time. The
// neutral stylesheet (kind → color/shape, relation → arrowhead
// category, provenance → width/opacity) lives below; swapping
// rendering libraries replaces this file without touching
// app.js / payload shape / chrome modules.

(function (global) {
  "use strict";

  // -- Neutral stylesheet ------------------------------------------
  // Mirrors src/output/dot.rs so the DOT and HTML exports stay
  // visually coherent.

  var NODE_KIND_STYLE = {
    repo: { color: "#c8e6c9", shape: "round-rectangle" },
    checkout: { color: "#a5d6a7", shape: "round-rectangle" },
    workspace: { color: "#b3e5fc", shape: "round-tag" },
    branch: { color: "#fff9c4", shape: "round-rectangle" },
    agent_session: { color: "#f8bbd0", shape: "round-rectangle" },
    mux_session: { color: "#d1c4e9", shape: "round-octagon" },
    fork: { color: "#ffe0b2", shape: "octagon" },
    forge_pr: { color: "#ffccbc", shape: "round-rectangle" },
    runtime_process: { color: "#eceff1", shape: "ellipse" },
    unresolved_stub: { color: "#ffffff", shape: "ellipse" },
  };

  var PROVENANCE_WIDTH = {
    local_declared: 3.0,
    global_declared: 3.0,
    strong_discovered: 2.0,
    discovered: 1.4,
    convention: 1.0,
    cached: 0.8,
  };

  var PROVENANCE_COLOR = {
    local_declared: "#1565c0",
    global_declared: "#1565c0",
    strong_discovered: "#212121",
    discovered: "#424242",
    convention: "#757575",
    cached: "#9e9e9e",
  };

  var RELATION_ARROW = {
    // Containment.
    workspace_contains_repo: "diamond",
    belongs_to_repo: "diamond",
    checked_out_branch: "diamond",
    mux_contains_process: "diamond",
    // Linkage between peers.
    linked_to_mux: "vee",
    associated_with: "vee",
    associated_branch: "vee",
    referenced_checkout: "vee",
    rooted_in: "vee",
    rooted_at_path: "vee",
    branch_has_forge_pr: "vee",
    // Lineage.
    parent_fork: "triangle",
    parent_session: "triangle",
    child_session: "triangle",
    forks_workspace: "triangle",
    forks_repo: "triangle",
    created_checkout: "triangle",
    created_branch: "triangle",
    // Runtime-process attribution.
    process_identifies_session: "circle",
    process_candidates_session: "circle",
  };

  // -- GraphDriver --------------------------------------------------

  function GraphDriver(container) {
    if (!global.cytoscape) {
      throw new Error("Cytoscape is not loaded");
    }
    // cytoscape-dagre does not self-register (unlike fcose, which
    // does when the global `cytoscape` is in scope at load time).
    // Register it here so the layout is available by name.
    if (
      global.cytoscapeDagre &&
      typeof global.cytoscapeDagre === "function" &&
      !global.cytoscape("layout", "dagre")
    ) {
      try { global.cytoscapeDagre(global.cytoscape); } catch (_) {}
    }
    this._cy = global.cytoscape({
      container: container,
      elements: [],
      layout: { name: "preset" },
      // Default styles attached at construction; load() rebuilds
      // element-keyed style classes after the payload arrives.
      style: this._buildBaseStylesheet(),
      wheelSensitivity: 0.2,
    });
    this._payload = null;
    this._currentLayout = "fcose";
    // Continuous density scale. 1.0 = compact baseline; presets
    // map to known values (see DENSITY_PRESETS). Anything off a
    // preset is "custom".
    this._densityScale = 1.6;
  }

  GraphDriver.prototype.load = function (payload) {
    this._payload = payload;
    var elements = this._payloadToElements(payload);
    this._cy.elements().remove();
    this._cy.add(elements);
    this._runLayout();
  };

  GraphDriver.prototype.payload = function () {
    return this._payload;
  };

  GraphDriver.prototype.focusNode = function (id) {
    var node = this._cy.getElementById(id);
    if (!node || node.empty()) return false;
    this._cy.animate(
      { fit: { eles: node, padding: 80 } },
      { duration: 250 }
    );
    return true;
  };

  GraphDriver.prototype.fit = function () {
    this._cy.fit(undefined, 40);
  };

  /// Fit the viewport to elements that are currently visible (not
  /// hidden by `.filtered-out`'s display:none). Used by navigation
  /// after a focus / depth / direction change so the user sees the
  /// resulting neighborhood rather than the single focused node.
  GraphDriver.prototype.fitVisible = function (padding) {
    var visible = this._cy.elements(":visible");
    if (visible.empty()) {
      this._cy.fit(undefined, padding || 40);
      return;
    }
    this._cy.fit(visible, padding || 40);
  };

  GraphDriver.prototype.onNodeClick = function (handler) {
    var driver = this;
    this._cy.on("tap", "node", function (event) {
      var n = event.target;
      handler({
        id: n.id(),
        data: n.data(),
        driver: driver,
      });
    });
  };

  GraphDriver.prototype.onBackgroundClick = function (handler) {
    this._cy.on("tap", function (event) {
      if (event.target === this) handler();
    });
  };

  /// Hide every element whose id is in `hiddenIds` (a Set or array).
  /// Implemented via a class so we can mass-toggle without touching
  /// per-element style. Edges with at least one hidden endpoint are
  /// hidden automatically by Cytoscape regardless of this call.
  GraphDriver.prototype.setHidden = function (hiddenIds) {
    var set =
      hiddenIds instanceof Set
        ? hiddenIds
        : new Set(Array.from(hiddenIds || []));
    this._cy.elements().forEach(function (ele) {
      if (set.has(ele.id())) {
        ele.addClass("filtered-out");
      } else {
        ele.removeClass("filtered-out");
      }
    });
  };

  /// Dim every element NOT in `keepIds` (a Set or array). Used by
  /// search: matches stay bright, the rest fade. Pass an empty set
  /// to clear the dim.
  GraphDriver.prototype.setDimmed = function (keepIds) {
    if (!keepIds || (keepIds.size === 0 && !Array.isArray(keepIds))) {
      this._cy.elements().removeClass("dimmed");
      return;
    }
    var set =
      keepIds instanceof Set ? keepIds : new Set(Array.from(keepIds));
    if (set.size === 0) {
      this._cy.elements().removeClass("dimmed");
      return;
    }
    this._cy.nodes().forEach(function (n) {
      if (set.has(n.id())) {
        n.removeClass("dimmed");
      } else {
        n.addClass("dimmed");
      }
    });
    // Edges follow: bright only if both endpoints are bright.
    this._cy.edges().forEach(function (e) {
      var ok =
        set.has(e.source().id()) && set.has(e.target().id());
      if (ok) e.removeClass("dimmed");
      else e.addClass("dimmed");
    });
  };

  /// Select a node programmatically and fire the selection handler.
  GraphDriver.prototype.selectNode = function (id) {
    var node = this._cy.getElementById(id);
    if (!node || node.empty()) return false;
    this._cy.elements().unselect();
    node.select();
    return true;
  };

  /// Subscribe to selection changes (any time selection set changes).
  GraphDriver.prototype.onSelectionChange = function (handler) {
    var driver = this;
    this._cy.on("select unselect", "node, edge", function () {
      var selected = driver._cy.elements(":selected");
      handler({
        ids: selected.map(function (e) {
          return e.id();
        }),
        elements: selected,
        driver: driver,
      });
    });
  };

  /// Return a structured detail object for the given node id:
  /// {node, incoming: [...edges], outgoing: [...edges]}. Edges are
  /// returned as plain payload-shaped objects with the neighbor
  /// node's label embedded for display. Chrome uses this to render
  /// the inspector panel without touching `cy` directly.
  GraphDriver.prototype.getDetail = function (id) {
    if (!this._payload) return null;
    var nodeRecord =
      this._payload.nodes.find(function (n) { return n.id === id; }) ||
      (this._payload.unresolved_stubs || []).find(function (s) {
        return s.id === id;
      });
    if (!nodeRecord) return null;

    // Index nodes + stubs for neighbor-label lookup.
    var labelById = {};
    this._payload.nodes.forEach(function (n) {
      labelById[n.id] = {
        kind: n.kind,
        label: n.label_primary || n.id,
        secondary: n.label_secondary || "",
      };
    });
    (this._payload.unresolved_stubs || []).forEach(function (s) {
      labelById[s.id] = {
        kind: "unresolved_stub",
        label: "?",
        secondary:
          (s.node_type || "?") +
          (s.harness_key ? ":" + s.harness_key : "") +
          (s.native_id ? ":" + s.native_id : ""),
      };
    });

    function withNeighbor(e, side) {
      var neighborId = side === "incoming" ? e.source : e.target;
      var info = labelById[neighborId] || {
        kind: "?",
        label: neighborId,
        secondary: "",
      };
      return Object.assign({}, e, { neighbor: info });
    }

    var incoming = [];
    var outgoing = [];
    this._payload.edges.forEach(function (e) {
      if (e.target === id) incoming.push(withNeighbor(e, "incoming"));
      if (e.source === id) outgoing.push(withNeighbor(e, "outgoing"));
    });

    // Group edges by relation for tidy rendering, preserving the
    // payload's deterministic edge order within each group.
    function group(edges) {
      var byRel = new Map();
      edges.forEach(function (e) {
        if (!byRel.has(e.relation)) byRel.set(e.relation, []);
        byRel.get(e.relation).push(e);
      });
      return Array.from(byRel.entries()).map(function (pair) {
        return { relation: pair[0], edges: pair[1] };
      });
    }

    return {
      node: nodeRecord,
      isStub: !!(this._payload.unresolved_stubs || []).find(function (s) {
        return s.id === id;
      }),
      incoming_groups: group(incoming),
      outgoing_groups: group(outgoing),
    };
  };

  // -- Public layout controls (GV-003d) --------------------------

  /// Available layout names in preference order. The driver picks
  /// the first one Cytoscape has registered when none is requested.
  GraphDriver.AVAILABLE_LAYOUTS = [
    { name: "fcose", label: "Force (fcose)" },
    { name: "dagre-lr", label: "Hierarchical →" },
    { name: "dagre-tb", label: "Hierarchical ↓" },
    { name: "cose", label: "Force (cose)" },
    { name: "concentric", label: "Concentric" },
    { name: "circle", label: "Circle" },
    { name: "grid", label: "Grid" },
  ];

  /// Return the names of layouts that Cytoscape can run right now
  /// (i.e. extensions actually loaded). Built-ins always present:
  /// cose, concentric, circle, grid, preset, null, random.
  GraphDriver.prototype.availableLayouts = function () {
    var self = this;
    return GraphDriver.AVAILABLE_LAYOUTS.filter(function (l) {
      var cyName = l.name.indexOf("dagre") === 0 ? "dagre" : l.name;
      return !!global.cytoscape("layout", cyName);
    });
  };

  GraphDriver.prototype.currentLayout = function () {
    return this._currentLayout;
  };

  /// Set the active layout and re-run it. Pass one of the names in
  /// availableLayouts() — `dagre-lr` and `dagre-tb` map to the
  /// `dagre` layout with rankdir LR / TB respectively.
  GraphDriver.prototype.setLayout = function (name) {
    this._currentLayout = name;
    this._runLayout();
  };

  /// Re-run the current layout (escapes local minima after a
  /// filter change or for a "Re-run" button).
  GraphDriver.prototype.rerunLayout = function () {
    this._runLayout();
  };

  /// Density presets. The chip group in the chrome maps these to
  /// numeric scale values, but the underlying control is the
  /// continuous scale on the slider — chips are just snap-to
  /// shortcuts. `Compact` = baseline (1.0); larger values spread
  /// nodes apart by scaling the positional layout parameters
  /// (idealEdgeLength, nodeRepulsion, nodeSeparation, …) and
  /// reducing center-pull (gravity).
  GraphDriver.AVAILABLE_DENSITIES = [
    { name: "compact", label: "Compact", scale: 1.0 },
    { name: "normal", label: "Normal", scale: 1.6 },
    { name: "spacious", label: "Spacious", scale: 2.5 },
  ];

  /// Allowed range for the slider. Going much past 8x produces
  /// graphs so spread out that pan-to-find becomes harder than
  /// the legibility win.
  GraphDriver.DENSITY_RANGE = { min: 0.5, max: 8.0, step: 0.1 };

  GraphDriver.prototype.densityScale = function () {
    return this._densityScale;
  };

  GraphDriver.prototype.setDensityScale = function (scale) {
    var n = parseFloat(scale);
    if (!isFinite(n)) return;
    var r = GraphDriver.DENSITY_RANGE;
    if (n < r.min) n = r.min;
    if (n > r.max) n = r.max;
    this._densityScale = n;
    this._runLayout();
  };

  /// Return the preset name whose scale matches the current
  /// scale (within tolerance), or "custom" when the slider has
  /// been dragged off any preset.
  GraphDriver.prototype.density = function () {
    var s = this._densityScale;
    for (var i = 0; i < GraphDriver.AVAILABLE_DENSITIES.length; i++) {
      var p = GraphDriver.AVAILABLE_DENSITIES[i];
      if (Math.abs(p.scale - s) < 0.05) return p.name;
    }
    return "custom";
  };

  /// Snap to a named preset (chip click).
  GraphDriver.prototype.setDensity = function (name) {
    for (var i = 0; i < GraphDriver.AVAILABLE_DENSITIES.length; i++) {
      var p = GraphDriver.AVAILABLE_DENSITIES[i];
      if (p.name === name) {
        this.setDensityScale(p.scale);
        return;
      }
    }
  };

  // -- Internals ----------------------------------------------------

  // Per-layout baseline parameters (= scale 1.0 = "compact"
  // density). Higher scale values multiply positional params and
  // dampen gravity. Adding a new layout means adding its row
  // here plus a case in _layoutOptions.
  var BASE_PROFILES = {
    fcose: {
      idealEdgeLength: 90,
      nodeRepulsion: 12000,
      nodeSeparation: 90,
      gravity: 0.4,
      edgeElasticity: 0.35,
    },
    dagre: { nodeSep: 40, rankSep: 80, edgeSep: 12 },
    cose: { idealEdgeLength: 90, nodeRepulsion: 10000 },
    concentric: { minNodeSpacing: 30 },
  };

  /// Apply the current density scale to a layout's baseline
  /// parameters. Most positional params scale linearly with the
  /// slider value; gravity decreases as 1/scale (more spread =
  /// less center pull) with a floor; edgeElasticity decreases
  /// more gently as 1/sqrt(scale).
  function densityFor(layoutKey, scale) {
    var base = BASE_PROFILES[layoutKey];
    if (!base) return {};
    var out = {};
    Object.keys(base).forEach(function (k) {
      var v = base[k];
      if (k === "gravity") {
        var g = v / scale;
        if (g < 0.03) g = 0.03;
        if (g > 0.6) g = 0.6;
        out[k] = g;
      } else if (k === "edgeElasticity") {
        var e = v / Math.sqrt(scale);
        if (e < 0.05) e = 0.05;
        out[k] = e;
      } else {
        out[k] = v * scale;
      }
    });
    return out;
  }

  GraphDriver.prototype._runLayout = function () {
    var name = this._currentLayout || "fcose";
    var opts = this._layoutOptions(name);
    if (!opts) opts = this._layoutOptions("cose");
    // Scope the layout to currently-visible elements so re-running
    // after a filter change actually re-flows around the remaining
    // graph. Hidden (display:none) elements keep their old positions
    // — they're invisible to both the user and the layout solver.
    // On initial load nothing is filtered yet, so this is a no-op.
    var visible = this._cy.elements(":visible");
    if (visible.empty()) {
      this._cy.layout(opts).run();
      return;
    }
    visible.layout(opts).run();
  };

  GraphDriver.prototype._layoutOptions = function (name) {
    var d = this._densityScale || 1.6;
    switch (name) {
      case "fcose": {
        if (!global.cytoscape("layout", "fcose")) return null;
        var f = densityFor("fcose", d);
        return {
          name: "fcose",
          quality: "proof",
          animate: false,
          randomize: true,
          nodeRepulsion: f.nodeRepulsion,
          idealEdgeLength: f.idealEdgeLength,
          edgeElasticity: f.edgeElasticity,
          gravity: f.gravity,
          gravityRange: 4.0,
          numIter: 2500,
          tile: true,
          tilingPaddingVertical: 12,
          tilingPaddingHorizontal: 12,
          padding: 40,
          nodeSeparation: f.nodeSeparation,
        };
      }
      case "dagre-lr":
      case "dagre-tb": {
        if (!global.cytoscape("layout", "dagre")) return null;
        var dg = densityFor("dagre", d);
        return {
          name: "dagre",
          rankDir: name === "dagre-lr" ? "LR" : "TB",
          nodeSep: dg.nodeSep,
          rankSep: dg.rankSep,
          edgeSep: dg.edgeSep,
          padding: 30,
          animate: false,
        };
      }
      case "concentric": {
        var co = densityFor("concentric", d);
        return {
          name: "concentric",
          animate: false,
          padding: 30,
          minNodeSpacing: co.minNodeSpacing,
          concentric: function (node) {
            // Inner ring: workspaces/forks (hubs). Outer rings:
            // checkouts/branches/sessions/processes. Stubs furthest.
            var k = node.data("kind");
            switch (k) {
              case "workspace": return 6;
              case "fork": return 5;
              case "repo": return 4;
              case "checkout":
              case "branch": return 3;
              case "agent_session":
              case "mux_session":
              case "forge_pr": return 2;
              case "runtime_process": return 1;
              default: return 0;
            }
          },
          levelWidth: function () { return 1; },
        };
      }
      case "cose": {
        var c = densityFor("cose", d);
        return {
          name: "cose",
          animate: false,
          randomize: true,
          nodeRepulsion: c.nodeRepulsion,
          idealEdgeLength: c.idealEdgeLength,
          padding: 40,
        };
      }
      case "circle":
        return { name: "circle", animate: false, padding: 30 };
      case "grid":
        return { name: "grid", animate: false, padding: 30 };
      default:
        return null;
    }
  };

  GraphDriver.prototype._payloadToElements = function (payload) {
    var elements = [];

    payload.nodes.forEach(function (n) {
      // Compose a two-line label: primary then a smaller secondary
      // context line. Cytoscape's text-wrap handles the rendering.
      var label = n.label_primary || n.id;
      if (n.label_secondary) {
        label += "\n" + n.label_secondary;
      }
      elements.push({
        group: "nodes",
        data: {
          id: n.id,
          kind: n.kind,
          label: label,
          label_primary: n.label_primary || "",
          label_secondary: n.label_secondary || "",
          attributes: n.attributes || {},
        },
        classes: "kind_" + n.kind,
      });
    });

    (payload.unresolved_stubs || []).forEach(function (s) {
      elements.push({
        group: "nodes",
        data: {
          id: s.id,
          kind: "unresolved_stub",
          label: "?",
          label_primary: "?",
          label_secondary:
            (s.node_type || "?") +
            (s.harness_key ? ":" + s.harness_key : "") +
            (s.native_id ? ":" + s.native_id : ""),
          unresolved: s,
        },
        classes: "kind_unresolved_stub",
      });
    });

    payload.edges.forEach(function (e) {
      var classes = [
        "rel_" + e.relation,
        "prov_" + e.provenance,
        e.is_resolved ? "resolved" : "candidate",
        "state_" + e.state,
      ];
      elements.push({
        group: "edges",
        data: {
          id: e.id,
          source: e.source,
          target: e.target,
          relation: e.relation,
          provenance: e.provenance,
          confidence: e.confidence,
          state: e.state,
          is_resolved: !!e.is_resolved,
          metadata: e.metadata || {},
          label: e.is_resolved
            ? e.relation + " ★"
            : e.relation,
        },
        classes: classes.join(" "),
      });
    });

    return elements;
  };

  GraphDriver.prototype._buildBaseStylesheet = function () {
    // Fixed node sizes. `width: "label"` interacts poorly with
    // several shape geometries in Cytoscape 3.33 — `round-tag`,
    // `octagon`, `round-octagon`, and some `round-rectangle` cases
    // end up with `.visible() === false` even though display /
    // visibility / opacity all look normal. Edges then hide because
    // Cytoscape hides edges with hidden endpoints. Fixed sizes are
    // the simplest robust fix and they let the label wrap inside.
    var styles = [
      {
        selector: "node",
        style: {
          "background-color": "#cfd8dc",
          "border-color": "#37474f",
          "border-width": 1,
          shape: "round-rectangle",
          label: "data(label)",
          "text-valign": "center",
          "text-halign": "center",
          "font-size": 11,
          "text-wrap": "wrap",
          "text-max-width": "120px",
          width: 130,
          height: 44,
          padding: "4px",
          color: "#212121",
        },
      },
      {
        selector: "edge",
        style: {
          width: 1.0,
          "line-color": "#424242",
          "target-arrow-color": "#424242",
          "target-arrow-shape": "vee",
          "curve-style": "bezier",
          "font-size": 9,
          label: "data(label)",
          "text-background-color": "#ffffff",
          "text-background-opacity": 0.85,
          "text-background-padding": 2,
          color: "#37474f",
        },
      },
      {
        selector: "edge.candidate",
        style: { "line-style": "dashed", opacity: 0.65 },
      },
      {
        selector: "edge.state_ignored, edge.state_overridden",
        style: {
          "line-color": "#e53935",
          "target-arrow-color": "#e53935",
          "line-style": "dashed",
          opacity: 0.7,
        },
      },
      // Filter/dim styling. `filtered-out` removes the element
      // entirely; `dimmed` fades it but keeps it visible so the
      // user can still see graph shape during search.
      {
        selector: ".filtered-out",
        style: { display: "none" },
      },
      {
        selector: "node.dimmed",
        style: { opacity: 0.18 },
      },
      {
        selector: "edge.dimmed",
        style: { opacity: 0.1 },
      },
      {
        selector: "node:selected",
        style: {
          "border-width": 3,
          "border-color": "#1565c0",
          "overlay-padding": 4,
          "overlay-opacity": 0.12,
          "overlay-color": "#1565c0",
        },
      },
    ];

    // Per-kind node styles.
    Object.keys(NODE_KIND_STYLE).forEach(function (kind) {
      var spec = NODE_KIND_STYLE[kind];
      styles.push({
        selector: "node.kind_" + kind,
        style: {
          "background-color": spec.color,
          shape: spec.shape,
        },
      });
    });

    // Unresolved-stub specific: small, dashed, no text padding.
    styles.push({
      selector: "node.kind_unresolved_stub",
      style: {
        "border-style": "dashed",
        "border-color": "#90a4ae",
        "font-size": 9,
        "text-max-width": "70px",
        width: 28,
        height: 28,
        padding: "2px",
      },
    });

    // Per-relation arrowheads.
    Object.keys(RELATION_ARROW).forEach(function (rel) {
      styles.push({
        selector: "edge.rel_" + rel,
        style: { "target-arrow-shape": RELATION_ARROW[rel] },
      });
    });

    // Per-provenance line width/color.
    Object.keys(PROVENANCE_WIDTH).forEach(function (prov) {
      styles.push({
        selector: "edge.prov_" + prov,
        style: {
          width: PROVENANCE_WIDTH[prov],
          "line-color": PROVENANCE_COLOR[prov],
          "target-arrow-color": PROVENANCE_COLOR[prov],
        },
      });
    });

    return styles;
  };

  // Expose the palette so chrome (e.g. the legend) can render
  // consistent swatches without re-encoding the encoding rules.
  GraphDriver.NODE_KIND_STYLE = NODE_KIND_STYLE;
  GraphDriver.PROVENANCE_WIDTH = PROVENANCE_WIDTH;
  GraphDriver.PROVENANCE_COLOR = PROVENANCE_COLOR;
  GraphDriver.RELATION_ARROW = RELATION_ARROW;

  global.ConspectusGraphDriver = GraphDriver;
})(window);
