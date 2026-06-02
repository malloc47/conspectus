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
      // GV-003e additions: recency / activity / orphan filters.
      maxAgeSeconds: null, // null = no max age
      hideStaleSessions: false,
      hideOrphans: false,
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
    panel.appendChild(this._renderRecencyGroup());
    panel.appendChild(this._renderActivityGroup());
    panel.appendChild(this._renderKindGroup());
    panel.appendChild(this._renderRelationGroup());
  };

  FilterPanel.prototype._renderRecencyGroup = function () {
    var section = el("div", "fp-section");
    section.appendChild(el("div", "fp-section-title", "Recency"));
    var options = [
      { label: "All", value: null },
      { label: "30d", value: 30 * 86400 },
      { label: "7d", value: 7 * 86400 },
      { label: "1d", value: 86400 },
      { label: "1h", value: 3600 },
    ];
    var row = el("div", "fp-chip-row");
    var self = this;
    options.forEach(function (opt) {
      var chip = el("button", "fp-chip", opt.label);
      if (opt.value === self.state.maxAgeSeconds)
        chip.classList.add("fp-chip-active");
      chip.title =
        opt.value == null
          ? "No recency filter"
          : "Hide nodes whose recency timestamp is older than " + opt.label;
      chip.addEventListener("click", function () {
        self.state.maxAgeSeconds = opt.value;
        self._render();
        self.apply();
      });
      row.appendChild(chip);
    });
    section.appendChild(row);
    var note = el(
      "div",
      "fp-section-note",
      "Applies per-kind: sessions, muxes, runtime processes, PRs. Structural nodes (repo, branch, …) ignore recency.",
    );
    section.appendChild(note);
    return section;
  };

  FilterPanel.prototype._renderActivityGroup = function () {
    var section = el("div", "fp-section");
    section.appendChild(el("div", "fp-section-title", "Activity"));
    var self = this;
    section.appendChild(
      checkbox(
        "Hide stale sessions (no process or mux)",
        this.state.hideStaleSessions,
        function (v) {
          self.state.hideStaleSessions = v;
          self.apply();
        },
      ),
    );
    section.appendChild(
      checkbox(
        "Hide orphan nodes (no visible edges)",
        this.state.hideOrphans,
        function (v) {
          self.state.hideOrphans = v;
          self.apply();
        },
      ),
    );
    return section;
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

    // Density controls — chips (snap-to presets) + continuous
    // slider for going past Spacious. Slider is the source of
    // truth; chips just snap it to known values.
    if (
      typeof window !== "undefined" &&
      window.ConspectusGraphDriver &&
      window.ConspectusGraphDriver.AVAILABLE_DENSITIES &&
      this.driver.setDensityScale
    ) {
      var densities = window.ConspectusGraphDriver.AVAILABLE_DENSITIES;
      var currentDensity = this.driver.density
        ? this.driver.density()
        : "custom";
      var currentScale =
        this.driver.densityScale != null
          ? this.driver.densityScale()
          : 1.6;
      var range =
        window.ConspectusGraphDriver.DENSITY_RANGE ||
        { min: 0.5, max: 8.0, step: 0.1 };

      var densityRow = el("div", "fp-chip-row fp-density-row");
      densities.forEach(function (opt) {
        var chip = el("button", "fp-chip", opt.label);
        if (opt.name === currentDensity) chip.classList.add("fp-chip-active");
        chip.title =
          "Density preset: " + opt.label.toLowerCase() + " (" + opt.scale + "x)";
        chip.addEventListener("click", function () {
          self.driver.setDensity(opt.name);
          self._render();
        });
        densityRow.appendChild(chip);
      });
      section.appendChild(densityRow);

      var sliderRow = el("div", "fp-density-slider-row");
      var slider = document.createElement("input");
      slider.type = "range";
      slider.className = "fp-density-slider";
      slider.min = String(range.min);
      slider.max = String(range.max);
      slider.step = String(range.step);
      slider.value = String(currentScale);
      slider.title = "Drag for fine control beyond the chip presets";
      var readout = el("span", "fp-density-value", formatScale(currentScale));
      // `input` fires continuously while dragging — re-running
      // fcose on every input would be choppy, so debounce to
      // `change` (which fires on mouseup) for the actual layout
      // re-run; update the readout live for feedback.
      slider.addEventListener("input", function () {
        readout.textContent = formatScale(parseFloat(slider.value));
      });
      slider.addEventListener("change", function () {
        self.driver.setDensityScale(parseFloat(slider.value));
        self._render();
      });
      sliderRow.appendChild(slider);
      sliderRow.appendChild(readout);
      section.appendChild(sliderRow);
    }

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

    // Recency filter: hide nodes whose per-kind recency epoch is
    // older than the threshold. Structural nodes (repo, branch,
    // workspace, fork, checkout) carry no recency signal and are
    // never hidden by this pass.
    if (s.maxAgeSeconds != null) {
      var nowS = Date.now() / 1000;
      payload.nodes.forEach(function (n) {
        var epoch = nodeRecencyEpoch(n);
        if (epoch == null) return;
        if (nowS - epoch > s.maxAgeSeconds) hidden.add(n.id);
      });
    }

    // Stale-session filter: hide AgentSession nodes that have no
    // visible edge of relation linked_to_mux /
    // process_identifies_session / process_candidates_session to a
    // visible neighbor. Sessions live in harness state files long
    // after their backing process exits; this prunes those.
    if (s.hideStaleSessions) {
      var activeRels = {
        linked_to_mux: true,
        process_identifies_session: true,
        process_candidates_session: true,
      };
      payload.nodes.forEach(function (n) {
        if (n.kind !== "agent_session") return;
        if (hidden.has(n.id)) return;
        var hasActive = payload.edges.some(function (e) {
          if (hidden.has(e.id)) return false;
          if (e.source !== n.id && e.target !== n.id) return false;
          var other = e.source === n.id ? e.target : e.source;
          if (hidden.has(other)) return false;
          return activeRels[e.relation] === true;
        });
        if (!hasActive) hidden.add(n.id);
      });
    }

    // Orphan filter: hide nodes with zero visible incident edges.
    // Runs LAST so it composes with every other filter.
    if (s.hideOrphans) {
      var degree = {};
      payload.nodes.forEach(function (n) {
        if (!hidden.has(n.id)) degree[n.id] = 0;
      });
      (payload.unresolved_stubs || []).forEach(function (stub) {
        if (!hidden.has(stub.id)) degree[stub.id] = 0;
      });
      payload.edges.forEach(function (e) {
        if (hidden.has(e.id)) return;
        if (degree[e.source] != null && degree[e.target] != null) {
          degree[e.source]++;
          degree[e.target]++;
        }
      });
      Object.keys(degree).forEach(function (id) {
        if (degree[id] === 0) hidden.add(id);
      });
    }

    if (this.viewState) {
      this.viewState.updateFilter(hidden);
    } else {
      this.driver.setHidden(hidden);
    }
  };

  /// Per-kind recency epoch. Returns the seconds-since-epoch value
  /// most appropriate to "is this node fresh", or `null` for kinds
  /// that have no meaningful recency signal (purely structural).
  function nodeRecencyEpoch(node) {
    var attrs = node.attributes || {};
    switch (node.kind) {
      case "agent_session":
        return attrs.last_active_epoch != null ? attrs.last_active_epoch : null;
      case "mux_session":
        return attrs.activity_epoch != null
          ? attrs.activity_epoch
          : attrs.created_epoch != null
            ? attrs.created_epoch
            : null;
      case "runtime_process":
        return attrs.observed_epoch != null ? attrs.observed_epoch : null;
      case "forge_pr":
        return attrs.updated_epoch != null ? attrs.updated_epoch : null;
      default:
        return null;
    }
  }

  // ----- DOM helpers --------------------------------------------

  /// Format a density scale for the slider readout. "1.6x".
  function formatScale(n) {
    if (!isFinite(n)) return "—";
    return n.toFixed(1) + "x";
  }

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
