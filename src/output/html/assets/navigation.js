// navigation.js — focus / depth / direction / breadcrumb chrome
// (GV-003c). Wraps the pure helpers from navigation-helpers.js
// and pushes hidden-id sets through ConspectusViewState so the
// filter panel and navigation compose cleanly.
//
// Per ADR 0050 decision 10: traversal runs over the neutral
// payload, never over Cytoscape's collection API.

(function (global) {
  "use strict";

  var DEPTH_OPTIONS = [
    { value: 1, label: "1" },
    { value: 2, label: "2" },
    { value: 3, label: "3" },
    { value: Infinity, label: "All" },
  ];
  var DIRECTION_OPTIONS = [
    { value: "both", label: "Both", title: "Both upstream and downstream" },
    { value: "up", label: "↑ Upstream", title: "Incoming edges only" },
    { value: "down", label: "↓ Downstream", title: "Outgoing edges only" },
  ];

  /// Renders a small bar above the canvas. Hidden when no focus is
  /// active; visible (with breadcrumb + chip groups + reset) when
  /// a node is focused. Listens for selection events to offer a
  /// "Focus selected" affordance via the inspector "Focus" button
  /// (wired by app.js).
  function ConspectusNavigation(opts) {
    this.host = opts.host;
    this.driver = opts.driver;
    this.viewState = opts.viewState;
    this.payload = this.driver.payload();
    this.onFocusChange = opts.onFocusChange || function () {};
    this.state = {
      focusId: null,
      depth: 2,
      direction: "both",
      history: [], // entries: {focusId, depth, direction}
    };
    this._labelById = buildLabelIndex(this.payload);
    this._render();
  }

  /// Public: focus on `nodeId`, pushing the current state onto the
  /// breadcrumb stack first (so Back returns to where the user was).
  ConspectusNavigation.prototype.focusOn = function (nodeId) {
    if (!nodeId) return;
    if (this.state.focusId === nodeId) {
      // Already focused — just re-apply (covers the case where the
      // user changed depth/direction and re-clicked).
      this._apply();
      return;
    }
    if (this.state.focusId) {
      this.state.history.push({
        focusId: this.state.focusId,
        depth: this.state.depth,
        direction: this.state.direction,
      });
    }
    this.state.focusId = nodeId;
    this._render();
    this._apply();
    this.onFocusChange(nodeId);
  };

  ConspectusNavigation.prototype.popBreadcrumb = function () {
    if (this.state.history.length === 0) {
      this.clear();
      return;
    }
    var prev = this.state.history.pop();
    this.state.focusId = prev.focusId;
    this.state.depth = prev.depth;
    this.state.direction = prev.direction;
    this._render();
    this._apply();
    this.onFocusChange(prev.focusId);
  };

  ConspectusNavigation.prototype.clear = function () {
    this.state.focusId = null;
    this.state.history = [];
    this._render();
    this.viewState.clearNav();
    this.onFocusChange(null);
  };

  ConspectusNavigation.prototype.setDepth = function (depth) {
    this.state.depth = depth;
    this._render();
    this._apply();
  };

  ConspectusNavigation.prototype.setDirection = function (direction) {
    this.state.direction = direction;
    this._render();
    this._apply();
  };

  ConspectusNavigation.prototype.isFocused = function () {
    return !!this.state.focusId;
  };

  ConspectusNavigation.prototype.currentFocusId = function () {
    return this.state.focusId;
  };

  // ---- internals ------------------------------------------------

  ConspectusNavigation.prototype._apply = function () {
    if (!this.state.focusId) {
      this.viewState.clearNav();
      return;
    }
    var visible = global.ConspectusNavHelpers.bfsNeighborhood(
      this.payload,
      this.state.focusId,
      this.state.depth,
      this.state.direction
    );
    var hidden = global.ConspectusNavHelpers.hiddenFromVisible(
      this.payload,
      visible
    );
    this.viewState.updateNav(hidden);

    // Fit to the currently-visible neighborhood (focused node +
    // everything within the current depth/direction restriction).
    // Calling focusNode(id) here would zoom to just the single
    // focused node, hiding the effect of depth/direction changes.
    if (this.driver.fitVisible) {
      this.driver.fitVisible(60);
    }
  };

  ConspectusNavigation.prototype._render = function () {
    if (!this.host) return;
    this.host.innerHTML = "";
    if (!this.state.focusId) {
      this.host.classList.remove("nav-active");
      return;
    }
    this.host.classList.add("nav-active");

    // Breadcrumb
    var crumbRow = el("div", "nav-crumb-row");
    var back = el("button", "nav-btn nav-back");
    back.textContent = "← Back";
    back.disabled = this.state.history.length === 0;
    back.title = "Pop the previous focus (Backspace)";
    var self = this;
    back.addEventListener("click", function () { self.popBreadcrumb(); });
    crumbRow.appendChild(back);

    this.state.history.forEach(function (h) {
      var crumb = el("span", "nav-crumb nav-crumb-historic");
      crumb.textContent = self._labelFor(h.focusId);
      crumb.title = h.focusId;
      crumbRow.appendChild(crumb);
      crumbRow.appendChild(el("span", "nav-crumb-sep", "›"));
    });
    var current = el("span", "nav-crumb nav-crumb-current");
    current.textContent = self._labelFor(this.state.focusId);
    current.title = this.state.focusId;
    crumbRow.appendChild(current);

    var clear = el("button", "nav-btn nav-clear", "Clear focus");
    clear.title = "Show the full graph (Esc)";
    clear.addEventListener("click", function () { self.clear(); });
    crumbRow.appendChild(clear);

    this.host.appendChild(crumbRow);

    // Chip groups (depth + direction)
    var chips = el("div", "nav-chips");
    chips.appendChild(el("span", "nav-chip-label", "Depth"));
    DEPTH_OPTIONS.forEach(function (opt) {
      var b = el("button", "nav-chip", opt.label);
      if (opt.value === self.state.depth) b.classList.add("nav-chip-active");
      b.addEventListener("click", function () { self.setDepth(opt.value); });
      chips.appendChild(b);
    });
    chips.appendChild(el("span", "nav-chip-sep"));
    chips.appendChild(el("span", "nav-chip-label", "Direction"));
    DIRECTION_OPTIONS.forEach(function (opt) {
      var b = el("button", "nav-chip", opt.label);
      b.title = opt.title;
      if (opt.value === self.state.direction) b.classList.add("nav-chip-active");
      b.addEventListener("click", function () { self.setDirection(opt.value); });
      chips.appendChild(b);
    });
    this.host.appendChild(chips);
  };

  ConspectusNavigation.prototype._labelFor = function (id) {
    var entry = this._labelById[id];
    return entry ? entry.label : id;
  };

  // ---- helpers --------------------------------------------------

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }

  function buildLabelIndex(payload) {
    var idx = {};
    (payload.nodes || []).forEach(function (n) {
      idx[n.id] = { label: n.label_primary || n.id, kind: n.kind };
    });
    (payload.unresolved_stubs || []).forEach(function (s) {
      idx[s.id] = {
        label: "?" + (s.native_id ? " " + s.native_id : ""),
        kind: "unresolved_stub",
      };
    });
    return idx;
  }

  global.ConspectusNavigation = ConspectusNavigation;
})(window);
