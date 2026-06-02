// app.js — bootstrap. Reads the embedded payload, constructs the
// GraphDriver, instantiates ConspectusFilterPanel and
// ConspectusInspector against the left/right slots, and wires the
// header search input to driver.setDimmed. All UI behavior lives
// in the per-feature modules; this file only composes them.

(function (global) {
  "use strict";

  var KIND_LABELS = {
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

  function readPayload() {
    var el = document.getElementById("conspectus-graph-payload");
    if (!el) throw new Error("missing #conspectus-graph-payload");
    return JSON.parse(el.textContent);
  }

  function setStatus(text) {
    var el = document.getElementById("conspectus-status");
    if (el) el.textContent = text;
  }

  function summarize(payload) {
    var byKind = {};
    payload.nodes.forEach(function (n) {
      byKind[n.kind] = (byKind[n.kind] || 0) + 1;
    });
    var parts = Object.keys(byKind)
      .sort()
      .map(function (k) {
        return k + ":" + byKind[k];
      })
      .join("  ");
    return (
      payload.nodes.length +
      " nodes (" +
      parts +
      ")  ·  " +
      payload.edges.length +
      " edges"
    );
  }

  /// Legend renderer passed to the inspector. Called with the right-
  /// pane host element whenever there's no selection.
  function legendRenderer(host) {
    var palette =
      (global.ConspectusGraphDriver &&
        global.ConspectusGraphDriver.NODE_KIND_STYLE) ||
      {};
    var legend = document.createElement("div");
    legend.className = "legend-inner";
    var html = '<div class="legend-title">Node kinds</div>';
    Object.keys(KIND_LABELS).forEach(function (kind) {
      var spec = palette[kind];
      if (!spec) return;
      html +=
        '<div class="legend-row"><span class="legend-swatch" style="background:' +
        spec.color +
        '"></span>' +
        KIND_LABELS[kind] +
        "</div>";
    });
    html += '<div class="legend-title legend-title-edge">Edges</div>';
    html +=
      '<div class="legend-row legend-row-edge"><span class="legend-edge legend-edge-solid"></span>resolved (★)</div>';
    html +=
      '<div class="legend-row legend-row-edge"><span class="legend-edge legend-edge-dashed"></span>candidate</div>';
    html +=
      '<div class="legend-row legend-row-edge"><span class="legend-edge legend-edge-red"></span>ignored / overridden</div>';
    legend.innerHTML = html;
    host.appendChild(legend);
  }

  /// Wire the header search input. On each keystroke, compute the
  /// id-set of payload nodes whose label or id substring-matches
  /// the (case-insensitive) query, plus the stub ids that match,
  /// then dim everything else via driver.setDimmed. Empty query
  /// clears the dim.
  function wireSearch(driver, payload) {
    var input = document.getElementById("conspectus-search");
    var counter = document.getElementById("conspectus-search-count");
    if (!input) return;
    input.addEventListener("input", function () {
      var q = input.value.trim().toLowerCase();
      if (!q) {
        driver.setDimmed(new Set());
        if (counter) counter.textContent = "";
        return;
      }
      var keep = new Set();
      payload.nodes.forEach(function (n) {
        if (
          (n.label_primary || "").toLowerCase().indexOf(q) !== -1 ||
          (n.label_secondary || "").toLowerCase().indexOf(q) !== -1 ||
          (n.kind || "").toLowerCase().indexOf(q) !== -1 ||
          (n.id || "").toLowerCase().indexOf(q) !== -1
        ) {
          keep.add(n.id);
        }
      });
      (payload.unresolved_stubs || []).forEach(function (s) {
        if (
          (s.node_type || "").toLowerCase().indexOf(q) !== -1 ||
          (s.id || "").toLowerCase().indexOf(q) !== -1
        ) {
          keep.add(s.id);
        }
      });
      driver.setDimmed(keep);
      if (counter) {
        counter.textContent = keep.size + " match" + (keep.size === 1 ? "" : "es");
      }
    });
  }

  function init() {
    var payload = readPayload();
    var container = document.getElementById("conspectus-cy");
    var driver = new global.ConspectusGraphDriver(container);
    driver.load(payload);
    driver.fit();
    setStatus(summarize(payload));

    var leftPane = document.getElementById("conspectus-left");
    var rightPane = document.getElementById("conspectus-right");

    if (leftPane && global.ConspectusFilterPanel) {
      new global.ConspectusFilterPanel(leftPane, driver);
    }
    var inspector = null;
    if (rightPane && global.ConspectusInspector) {
      inspector = new global.ConspectusInspector(
        rightPane,
        driver,
        legendRenderer
      );
    }

    wireSearch(driver, payload);

    // Selection routes to the inspector. Background-click clears.
    driver.onSelectionChange(function (sel) {
      if (!inspector) return;
      if (sel.ids.length === 1) {
        inspector.show(sel.ids[0]);
      } else {
        inspector.clear();
      }
    });
    driver.onBackgroundClick(function () {
      setStatus(summarize(payload));
      if (inspector) inspector.clear();
    });

    global.conspectus = {
      driver: driver,
      payload: payload,
      inspector: inspector,
    };
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})(window);
