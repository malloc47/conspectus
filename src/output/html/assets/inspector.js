// inspector.js — right-side detail panel (GV-003b).
//
// When a node is selected, replaces the legend with that node's
// attributes plus grouped incoming/outgoing edges. Clicking an
// edge row focuses the neighbor on the canvas and re-targets the
// inspector. Walks the neutral payload directly per ADR 0050
// decision 10 (no cy.collection().neighborhood()).

(function (global) {
  "use strict";

  var KIND_LABEL = {
    workspace: "Workspace",
    repo: "Repo",
    checkout: "Checkout",
    branch: "Branch",
    fork: "Fork",
    agent_session: "Agent session",
    mux_session: "Mux session",
    runtime_process: "Runtime process",
    forge_pr: "Forge PR",
    unresolved_stub: "Unresolved endpoint",
  };

  /// `onFocusRequest` is an optional callback fired when the user
  /// clicks the inspector "Focus" button (GV-003c). When supplied,
  /// the header gains the button; otherwise it's omitted so the
  /// inspector can still run without navigation chrome.
  function Inspector(host, driver, legendRenderer, onFocusRequest) {
    this.host = host;
    this.driver = driver;
    this.payload = driver.payload();
    this.legendRenderer = legendRenderer;
    this.onFocusRequest = onFocusRequest || null;
    this._index();
    this.clear();
  }

  Inspector.prototype._index = function () {
    var p = this.payload;
    this.nodesById = {};
    p.nodes.forEach(function (n) {
      this.nodesById[n.id] = n;
    }, this);
    (p.unresolved_stubs || []).forEach(function (s) {
      this.nodesById[s.id] = {
        id: s.id,
        kind: "unresolved_stub",
        label_primary: "?",
        label_secondary:
          (s.node_type || "?") +
          (s.harness_key ? ":" + s.harness_key : "") +
          (s.native_id ? ":" + s.native_id : ""),
        attributes: s,
      };
    }, this);
    // Per-node edge groupings.
    this.outgoingBySource = {};
    this.incomingByTarget = {};
    p.edges.forEach(function (e) {
      (this.outgoingBySource[e.source] =
        this.outgoingBySource[e.source] || []).push(e);
      (this.incomingByTarget[e.target] =
        this.incomingByTarget[e.target] || []).push(e);
    }, this);
  };

  /// Show the inspector for a node id. Falls back to clear() if id
  /// is null/undefined or unknown.
  Inspector.prototype.show = function (id) {
    var node = id && this.nodesById[id];
    if (!node) {
      this.clear();
      return;
    }
    this.host.innerHTML = "";

    this.host.appendChild(this._header(node));
    this.host.appendChild(this._attributes(node));

    var outgoing = (this.outgoingBySource[id] || []).slice();
    var incoming = (this.incomingByTarget[id] || []).slice();
    if (outgoing.length > 0) {
      this.host.appendChild(this._edgeSection("Outgoing", outgoing, "target"));
    }
    if (incoming.length > 0) {
      this.host.appendChild(this._edgeSection("Incoming", incoming, "source"));
    }
    if (outgoing.length === 0 && incoming.length === 0) {
      this.host.appendChild(
        elText("div", "ins-empty", "No edges incident to this node.")
      );
    }
  };

  /// Restore the legend (default empty-selection state).
  Inspector.prototype.clear = function () {
    this.host.innerHTML = "";
    if (this.legendRenderer) this.legendRenderer(this.host);
  };

  // ---- subviews ------------------------------------------------

  Inspector.prototype._header = function (node) {
    var hdr = el("div", "ins-header");
    var swatch = el("span", "ins-swatch ins-swatch-" + node.kind);
    hdr.appendChild(swatch);
    var kindLine = elText(
      "div",
      "ins-kind",
      KIND_LABEL[node.kind] || node.kind
    );
    var labelLine = elText(
      "div",
      "ins-label",
      node.label_primary || node.id
    );
    var subLine = elText(
      "div",
      "ins-sublabel",
      node.label_secondary || ""
    );
    var idLine = elText("div", "ins-id", node.id);
    var col = el("div", "ins-header-col");
    col.appendChild(kindLine);
    col.appendChild(labelLine);
    if (node.label_secondary) col.appendChild(subLine);
    col.appendChild(idLine);
    hdr.appendChild(col);

    if (this.onFocusRequest) {
      var btn = document.createElement("button");
      btn.className = "ins-focus-btn";
      btn.textContent = "Focus";
      btn.title =
        "Restrict the view to this node and its neighbors within the current depth";
      var self = this;
      btn.addEventListener("click", function () {
        self.onFocusRequest(node.id);
      });
      hdr.appendChild(btn);
    }
    return hdr;
  };

  Inspector.prototype._attributes = function (node) {
    var box = el("div", "ins-attrs");
    box.appendChild(elText("div", "ins-section-title", "Attributes"));
    var rows = flattenAttributes(node.attributes);
    if (rows.length === 0) {
      box.appendChild(elText("div", "ins-empty", "(no attributes)"));
      return box;
    }
    var tbl = el("div", "ins-kv");
    rows.forEach(function (kv) {
      var k = elText("div", "ins-k", kv.key);
      var v = elText("div", "ins-v", kv.value);
      tbl.appendChild(k);
      tbl.appendChild(v);
    });
    box.appendChild(tbl);
    return box;
  };

  Inspector.prototype._edgeSection = function (title, edges, otherEnd) {
    var section = el("div", "ins-edges");
    section.appendChild(
      elText("div", "ins-section-title", title + " (" + edges.length + ")")
    );
    // Group by relation.
    var byRelation = {};
    edges.forEach(function (e) {
      (byRelation[e.relation] = byRelation[e.relation] || []).push(e);
    });
    var relations = Object.keys(byRelation).sort();
    var self = this;
    relations.forEach(function (rel) {
      var group = el("div", "ins-edge-group");
      group.appendChild(elText("div", "ins-edge-relation", rel));
      byRelation[rel].forEach(function (e) {
        group.appendChild(self._edgeRow(e, otherEnd));
      });
      section.appendChild(group);
    });
    return section;
  };

  Inspector.prototype._edgeRow = function (e, otherEnd) {
    var row = el("div", "ins-edge-row");
    var otherId = e[otherEnd];
    var other = this.nodesById[otherId];
    var otherLabel = other ? other.label_primary || otherId : otherId;
    var otherKind = other ? other.kind : "unknown";

    var swatch = el(
      "span",
      "ins-swatch ins-swatch-small ins-swatch-" + otherKind
    );
    row.appendChild(swatch);

    var col = el("div", "ins-edge-col");
    col.appendChild(elText("div", "ins-edge-other", otherLabel));
    var meta = e.provenance + " · " + e.confidence;
    if (e.is_resolved) meta = "★ " + meta;
    if (e.state !== "active") meta += " · " + e.state;
    col.appendChild(elText("div", "ins-edge-meta", meta));
    row.appendChild(col);

    if (other) {
      row.classList.add("ins-edge-clickable");
      var self = this;
      row.addEventListener("click", function () {
        self.driver.selectNode(otherId);
        self.driver.focusNode(otherId);
        self.show(otherId);
      });
    }
    return row;
  };

  // ---- helpers -------------------------------------------------

  function el(tag, cls) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    return e;
  }
  function elText(tag, cls, text) {
    var e = el(tag, cls);
    e.textContent = text;
    return e;
  }

  /// Flatten `attributes` into key:value pairs. Nested objects use
  /// dotted paths. `type` and `id` (the redundant kind/id repeats)
  /// are dropped because the header already shows them.
  function flattenAttributes(attrs, prefix) {
    var out = [];
    if (!attrs || typeof attrs !== "object") return out;
    Object.keys(attrs)
      .sort()
      .forEach(function (k) {
        if ((prefix == null || prefix === "") && (k === "type" || k === "id"))
          return;
        var path = prefix ? prefix + "." + k : k;
        var v = attrs[k];
        if (v == null) return;
        if (Array.isArray(v)) {
          if (v.length === 0) return;
          out.push({ key: path, value: v.join(", ") });
        } else if (typeof v === "object") {
          out = out.concat(flattenAttributes(v, path));
        } else {
          out.push({ key: path, value: String(v) });
        }
      });
    return out;
  }

  global.ConspectusInspector = Inspector;
})(window);
