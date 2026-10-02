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
    // Per-node edge groupings, plus an id-keyed edge index for
    // edge selection.
    this.outgoingBySource = {};
    this.incomingByTarget = {};
    this.edgesById = {};
    p.edges.forEach(function (e) {
      (this.outgoingBySource[e.source] =
        this.outgoingBySource[e.source] || []).push(e);
      (this.incomingByTarget[e.target] =
        this.incomingByTarget[e.target] || []).push(e);
      this.edgesById[e.id] = e;
    }, this);
  };

  /// Show the inspector for a node OR edge id. Routes to the
  /// appropriate renderer; falls back to clear() if the id is
  /// unknown.
  Inspector.prototype.show = function (id) {
    if (!id) {
      this.clear();
      return;
    }
    var node = this.nodesById[id];
    if (node) {
      this._showNode(id, node);
      return;
    }
    var edge = this.edgesById[id];
    if (edge) {
      this._showEdge(edge);
      return;
    }
    this.clear();
  };

  Inspector.prototype._showNode = function (id, node) {
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

  /// Render the inspector for an edge. Shows the edge's full
  /// metadata (provenance / confidence / state / freshness /
  /// adapter / evidence / fields), its endpoints with click-to-
  /// navigate, and — for resolver-preferred candidates — a side-
  /// by-side row of every losing candidate the resolver evaluated
  /// for the same (source, relation, target) so the operator can
  /// see exactly why this edge won.
  Inspector.prototype._showEdge = function (edge) {
    this.host.innerHTML = "";
    this.host.classList.add("ins-edge-mode");

    // Header
    var hdr = el("div", "ins-header ins-edge-header");
    var col = el("div", "ins-header-col");
    col.appendChild(elText("div", "ins-kind", "Edge"));
    var title = edge.relation + (edge.is_resolved ? " ★" : "");
    col.appendChild(elText("div", "ins-label", title));
    col.appendChild(
      elText(
        "div",
        "ins-sublabel",
        edge.provenance + " · " + edge.confidence + " · " + edge.state
      )
    );
    col.appendChild(elText("div", "ins-id", edge.id));
    hdr.appendChild(col);
    this.host.appendChild(hdr);

    // Endpoints (clickable to navigate)
    this.host.appendChild(this._edgeEndpointsSection(edge));

    // State detail (ignored / overridden)
    if (edge.state_detail) {
      this.host.appendChild(this._edgeStateSection(edge));
    }

    // Mechanism (provenance / confidence / freshness / adapter /
    // evidence / fields)
    this.host.appendChild(this._edgeMechanismSection(edge));

    // Competing and corroborating candidates (only for resolver winners)
    if (edge.is_resolved && edge.competing_link_ids && edge.competing_link_ids.length > 0) {
      this.host.appendChild(
        this._edgeCandidateSection(
          edge.competing_link_ids,
          "Lost candidates",
          "The resolver chose this edge over the candidates below, which name a different target. Compare provenance and adapter fields side by side."
        )
      );
    }
    if (
      edge.is_resolved &&
      edge.corroborating_link_ids &&
      edge.corroborating_link_ids.length > 0
    ) {
      this.host.appendChild(
        this._edgeCandidateSection(
          edge.corroborating_link_ids,
          "Corroborating candidates",
          "These candidates name the same target as this edge. The resolver ranked this edge first; the others agree with it."
        )
      );
    }
  };

  /// Restore the legend (default empty-selection state).
  Inspector.prototype.clear = function () {
    this.host.innerHTML = "";
    this.host.classList.remove("ins-edge-mode");
    if (this.legendRenderer) this.legendRenderer(this.host);
  };

  // ---- edge subviews -------------------------------------------

  Inspector.prototype._edgeEndpointsSection = function (edge) {
    var section = el("div", "ins-edges");
    section.appendChild(elText("div", "ins-section-title", "Endpoints"));
    var self = this;
    function row(label, otherId) {
      var other = self.nodesById[otherId];
      var otherLabel = other ? other.label_primary || otherId : otherId;
      var otherKind = other ? other.kind : "unknown";
      var r = el("div", "ins-edge-row ins-edge-clickable");
      r.appendChild(el("span", "ins-swatch ins-swatch-small ins-swatch-" + otherKind));
      var c = el("div", "ins-edge-col");
      c.appendChild(elText("div", "ins-edge-relation", label));
      c.appendChild(elText("div", "ins-edge-other", otherLabel));
      r.appendChild(c);
      r.addEventListener("click", function () {
        self.driver.selectNode(otherId);
        self.driver.focusNode(otherId);
        self.show(otherId);
      });
      return r;
    }
    section.appendChild(row("Source", edge.source));
    section.appendChild(row("Target", edge.target));
    return section;
  };

  Inspector.prototype._edgeStateSection = function (edge) {
    var sd = edge.state_detail;
    var section = el("div", "ins-edges ins-state-" + sd.kind);
    section.appendChild(
      elText("div", "ins-section-title", sd.kind === "ignored" ? "Ignored" : "Overridden")
    );
    var rows = [];
    if (sd.kind === "overridden" && sd.overridden_by) {
      rows.push({ key: "by", value: sd.overridden_by });
    }
    if (sd.reason) {
      rows.push({ key: "reason", value: sd.reason });
    }
    if (rows.length === 0) {
      section.appendChild(elText("div", "ins-empty", "(no detail provided)"));
      return section;
    }
    var tbl = el("div", "ins-kv");
    rows.forEach(function (kv) {
      tbl.appendChild(elText("div", "ins-k", kv.key));
      tbl.appendChild(elText("div", "ins-v", kv.value));
    });
    section.appendChild(tbl);
    return section;
  };

  Inspector.prototype._edgeMechanismSection = function (edge) {
    var section = el("div", "ins-edges");
    section.appendChild(elText("div", "ins-section-title", "Mechanism"));
    var rows = [
      { key: "provenance", value: edge.provenance },
      { key: "confidence", value: edge.confidence },
      { key: "freshness", value: edge.freshness },
    ];
    var md = edge.metadata || {};
    if (md.adapter) rows.push({ key: "adapter", value: md.adapter });
    if (md.evidence) rows.push({ key: "evidence", value: md.evidence });
    var tbl = el("div", "ins-kv");
    rows.forEach(function (kv) {
      tbl.appendChild(elText("div", "ins-k", kv.key));
      tbl.appendChild(elText("div", "ins-v", String(kv.value)));
    });
    section.appendChild(tbl);

    // Adapter fields (arbitrary key/value)
    var fields = (md && md.fields) || {};
    var fieldKeys = Object.keys(fields).sort();
    if (fieldKeys.length > 0) {
      section.appendChild(elText("div", "ins-subsection-title", "Adapter fields"));
      var ftbl = el("div", "ins-kv");
      fieldKeys.forEach(function (k) {
        ftbl.appendChild(elText("div", "ins-k", k));
        ftbl.appendChild(
          elText("div", "ins-v", typeof fields[k] === "object"
            ? JSON.stringify(fields[k])
            : String(fields[k]))
        );
      });
      section.appendChild(ftbl);
    }
    return section;
  };

  Inspector.prototype._edgeCandidateSection = function (ids, title, noteText) {
    var section = el("div", "ins-edges");
    section.appendChild(
      elText("div", "ins-section-title", title + " (" + ids.length + ")")
    );
    var note = el("div", "ins-section-note", noteText);
    section.appendChild(note);
    var self = this;
    ids.forEach(function (lostId) {
      var lost = self.edgesById[lostId];
      var row = el("div", "ins-edge-row ins-edge-clickable");
      var col = el("div", "ins-edge-col");
      if (lost) {
        col.appendChild(
          elText(
            "div",
            "ins-edge-other",
            lost.provenance + " · " + lost.confidence + " · " + lost.state
          )
        );
        var subText = "→ " + (self._labelFor(lost.target) || lost.target);
        col.appendChild(elText("div", "ins-edge-meta", subText));
      } else {
        col.appendChild(elText("div", "ins-edge-other", lostId));
        col.appendChild(
          elText("div", "ins-edge-meta", "(not in current view — filtered out)")
        );
      }
      row.appendChild(col);
      if (lost) {
        row.addEventListener("click", function () {
          self.driver.selectNode(lostId);
          self.show(lostId);
        });
      } else {
        row.classList.remove("ins-edge-clickable");
      }
      section.appendChild(row);
    });
    return section;
  };

  Inspector.prototype._labelFor = function (id) {
    var n = this.nodesById[id];
    return n ? n.label_primary || id : id;
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
