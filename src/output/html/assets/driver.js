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

  // -- Internals ----------------------------------------------------

  GraphDriver.prototype._runLayout = function () {
    var hasFcose =
      global.cytoscape && global.cytoscape("layout", "fcose");
    var opts = hasFcose
      ? {
          name: "fcose",
          quality: "proof",
          animate: false,
          randomize: true,
          // Higher repulsion + shorter ideal edge length pulls
          // connected clusters together while keeping disconnected
          // components apart.
          nodeRepulsion: 12000,
          idealEdgeLength: 90,
          edgeElasticity: 0.35,
          gravity: 0.4,
          gravityRange: 4.0,
          numIter: 2500,
          tile: true,
          tilingPaddingVertical: 12,
          tilingPaddingHorizontal: 12,
          padding: 40,
          nodeSeparation: 90,
        }
      : {
          name: "cose",
          animate: false,
          randomize: true,
          nodeRepulsion: 10000,
          idealEdgeLength: 90,
          padding: 40,
        };
    this._cy.layout(opts).run();
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
