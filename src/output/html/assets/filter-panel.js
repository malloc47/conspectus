// filter-panel.js — left-side controls (GV-003b).
//
// Computes a hidden-ids set from the neutral payload and pushes it
// to the driver. All hide logic runs over `payload`, not over the
// underlying Cytoscape instance, per ADR 0050 decision 10.

(function (global) {
  "use strict";

  // Human-readable labels. Anything not listed falls through to the
  // raw snake_case tag.
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
    unresolved_stub: "Unresolved",
  };
  var RELATION_LABEL = {
    workspace_contains_repo: "workspace contains repo",
    belongs_to_repo: "belongs to repo",
    checked_out_branch: "checked-out branch",
    branch_has_forge_pr: "branch ↔ PR",
    linked_to_mux: "linked to mux",
    associated_with: "associated with",
    associated_branch: "associated branch",
    referenced_checkout: "referenced checkout",
    rooted_in: "rooted in",
    rooted_at_path: "rooted at path",
    forks_workspace: "forks workspace",
    forks_repo: "forks repo",
    created_checkout: "created checkout",
    created_branch: "created branch",
    parent_fork: "parent fork",
    parent_session: "parent session",
    child_session: "child session",
    mux_contains_process: "mux contains process",
    process_identifies_session: "process identifies session",
    process_candidates_session: "process candidates session",
  };

  /// `viewState` is optional. When supplied, the panel pushes its
  /// hidden-id set through the coordinator so it composes with the
  /// navigation layer (GV-003c). When omitted, the panel drives the
  /// `setHidden` call directly for backwards compatibility.
  function FilterPanel(host, driver, viewState) {
    this.host = host;
    this.driver = driver;
    this.viewState = viewState || null;
    this.payload = driver.payload();
    this.state = this._defaultState();
    this._render();
    this.apply();
  }

  FilterPanel.prototype._defaultState = function () {
    // Defaults per ADR 0050: resolved view, runtime + stubs +
    // ignored visible (the user can collapse via the preset). Per-
    // kind / per-relation all on.
    var kinds = uniqueKinds(this.payload);
    var relations = uniqueRelations(this.payload);
    var kindOn = {};
    kinds.forEach(function (k) {
      kindOn[k] = true;
    });
    var relOn = {};
    relations.forEach(function (r) {
      relOn[r] = true;
    });
    return {
      showCandidates: true,
      showRuntimeProcess: true,
      showUnresolvedStubs: true,
      showIgnoredOverridden: true,
      kindOn: kindOn,
      relationOn: relOn,
      kinds: kinds,
      relations: relations,
    };
  };

  FilterPanel.prototype._render = function () {
    var s = this.state;
    var panel = this.host;
    panel.innerHTML = "";

    var titleBar = el("div", "fp-titlebar");
    titleBar.appendChild(el("span", "fp-title", "Filters"));
    var preset = el("button", "fp-preset", "Collapsed view");
    preset.title =
      "Hide candidates, runtime processes, unresolved stubs, and ignored/overridden links — approximates what the CLI/TUI shows.";
    preset.addEventListener("click", this._applyCollapsed.bind(this));
    titleBar.appendChild(preset);
    var reset = el("button", "fp-preset fp-reset", "Reset");
    reset.addEventListener("click", this._applyDefault.bind(this));
    titleBar.appendChild(reset);
    panel.appendChild(titleBar);

    panel.appendChild(this._renderTopToggles());
    panel.appendChild(this._renderLayoutGroup());
    panel.appendChild(this._renderKindGroup());
    panel.appendChild(this._renderRelationGroup());
  };

  FilterPanel.prototype._renderLayoutGroup = function () {
    var section = el("div", "fp-section");
    section.appendChild(el("div", "fp-section-title", "Layout"));
    if (!this.driver.availableLayouts || !this.driver.setLayout) {
      return section; // older driver — no layout switching available
    }
    var available = this.driver.availableLayouts();
    if (available.length <= 1) return section;
    var current = this.driver.currentLayout
      ? this.driver.currentLayout()
      : null;

    var select = document.createElement("select");
    select.className = "fp-layout-select";
    var self = this;
    available.forEach(function (opt) {
      var o = document.createElement("option");
      o.value = opt.name;
      o.textContent = opt.label;
      if (opt.name === current) o.selected = true;
      select.appendChild(o);
    });
    select.addEventListener("change", function () {
      self.driver.setLayout(select.value);
    });
    section.appendChild(select);

    var rerun = el("button", "fp-rerun", "Re-run layout");
    rerun.title = "Re-run the current layout (escapes local minima)";
    rerun.addEventListener("click", function () { self.driver.rerunLayout(); });
    section.appendChild(rerun);
    return section;
  };

  FilterPanel.prototype._renderTopToggles = function () {
    var section = el("div", "fp-section");
    section.appendChild(el("div", "fp-section-title", "View"));
    var self = this;
    section.appendChild(
      checkbox("Show candidate links", this.state.showCandidates, function (v) {
        self.state.showCandidates = v;
        self.apply();
      })
    );
    section.appendChild(
      checkbox(
        "Show runtime processes",
        this.state.showRuntimeProcess,
        function (v) {
          self.state.showRuntimeProcess = v;
          self.apply();
        }
      )
    );
    section.appendChild(
      checkbox(
        "Show unresolved endpoints",
        this.state.showUnresolvedStubs,
        function (v) {
          self.state.showUnresolvedStubs = v;
          self.apply();
        }
      )
    );
    section.appendChild(
      checkbox(
        "Show ignored / overridden",
        this.state.showIgnoredOverridden,
        function (v) {
          self.state.showIgnoredOverridden = v;
          self.apply();
        }
      )
    );
    return section;
  };

  FilterPanel.prototype._renderKindGroup = function () {
    var section = el("div", "fp-section");
    section.appendChild(el("div", "fp-section-title", "Node kinds"));
    var self = this;
    this.state.kinds.forEach(function (kind) {
      section.appendChild(
        checkbox(
          KIND_LABEL[kind] || kind,
          self.state.kindOn[kind],
          function (v) {
            self.state.kindOn[kind] = v;
            self.apply();
          }
        )
      );
    });
    return section;
  };

  FilterPanel.prototype._renderRelationGroup = function () {
    var section = el("div", "fp-section");
    section.appendChild(el("div", "fp-section-title", "Relations"));
    var self = this;
    this.state.relations.forEach(function (rel) {
      section.appendChild(
        checkbox(
          RELATION_LABEL[rel] || rel,
          self.state.relationOn[rel],
          function (v) {
            self.state.relationOn[rel] = v;
            self.apply();
          }
        )
      );
    });
    return section;
  };

  FilterPanel.prototype._applyCollapsed = function () {
    this.state.showCandidates = false;
    this.state.showRuntimeProcess = false;
    this.state.showUnresolvedStubs = false;
    this.state.showIgnoredOverridden = false;
    this._render();
    this.apply();
  };

  FilterPanel.prototype._applyDefault = function () {
    this.state = this._defaultState();
    this._render();
    this.apply();
  };

  /// Recompute the hidden set from current state and push to driver.
  FilterPanel.prototype.apply = function () {
    var s = this.state;
    var payload = this.payload;
    var hidden = new Set();

    // Nodes: hide by kind toggle or per-kind checkbox.
    payload.nodes.forEach(function (n) {
      if (!s.kindOn[n.kind]) hidden.add(n.id);
      if (n.kind === "runtime_process" && !s.showRuntimeProcess) {
        hidden.add(n.id);
      }
    });
    (payload.unresolved_stubs || []).forEach(function (stub) {
      if (!s.showUnresolvedStubs) hidden.add(stub.id);
    });

    // Edges: hide by relation toggle, candidate toggle, state toggle.
    payload.edges.forEach(function (e) {
      var hide = false;
      if (!s.relationOn[e.relation]) hide = true;
      if (!s.showCandidates && !e.is_resolved) hide = true;
      if (
        !s.showIgnoredOverridden &&
        (e.state === "ignored" || e.state === "overridden")
      ) {
        hide = true;
      }
      // Edge inherits hidden state from endpoints; Cytoscape handles
      // that automatically once both endpoint ids are in the set,
      // but we also explicitly hide the edge itself for clarity.
      if (hide) hidden.add(e.id);
    });

    if (this.viewState) {
      this.viewState.updateFilter(hidden);
    } else {
      this.driver.setHidden(hidden);
    }
  };

  // ----- DOM helpers --------------------------------------------

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }

  function checkbox(label, initial, onchange) {
    var row = el("label", "fp-checkbox");
    var input = document.createElement("input");
    input.type = "checkbox";
    input.checked = !!initial;
    input.addEventListener("change", function () {
      onchange(input.checked);
    });
    row.appendChild(input);
    row.appendChild(el("span", "fp-checkbox-label", label));
    return row;
  }

  function uniqueKinds(payload) {
    var seen = new Set();
    payload.nodes.forEach(function (n) {
      seen.add(n.kind);
    });
    if ((payload.unresolved_stubs || []).length > 0) seen.add("unresolved_stub");
    // Stable, kind-order-aware sort.
    var order = [
      "workspace",
      "repo",
      "checkout",
      "branch",
      "fork",
      "agent_session",
      "mux_session",
      "runtime_process",
      "forge_pr",
      "unresolved_stub",
    ];
    return Array.from(seen).sort(function (a, b) {
      return order.indexOf(a) - order.indexOf(b);
    });
  }

  function uniqueRelations(payload) {
    var seen = new Set();
    payload.edges.forEach(function (e) {
      seen.add(e.relation);
    });
    return Array.from(seen).sort();
  }

  global.ConspectusFilterPanel = FilterPanel;
})(window);
