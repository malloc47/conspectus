// navigation-helpers.js — pure graph-traversal helpers for the
// GV-003c navigation chrome.
//
// All traversal runs over the library-neutral payload (per ADR
// 0050 decision 10 Coupling Boundary), never over Cytoscape's
// collection API. The helpers are deliberately small and side-
// effect-free so a future swap to a different rendering library
// reuses them verbatim.

(function (global) {
  "use strict";

  /// Build a directed adjacency index over the payload edges and
  /// any unresolved-endpoint stubs. Returns `{outgoing, incoming}`
  /// where each is `Map<nodeId, Set<nodeId>>`. Edges whose target
  /// is a stub id are included (stubs are first-class neighbors
  /// for traversal purposes).
  function buildAdjacency(payload) {
    var outgoing = new Map();
    var incoming = new Map();
    function add(map, from, to) {
      var set = map.get(from);
      if (!set) {
        set = new Set();
        map.set(from, set);
      }
      set.add(to);
    }
    (payload.edges || []).forEach(function (e) {
      add(outgoing, e.source, e.target);
      add(incoming, e.target, e.source);
    });
    return { outgoing: outgoing, incoming: incoming };
  }

  /// BFS from `focusId` over the payload graph. Returns a Set of
  /// node ids reachable within `depth` hops, including `focusId`.
  ///
  /// - `direction`: "both" (treat the graph as undirected),
  ///   "down" (follow outgoing edges only), or "up" (follow
  ///   incoming edges only).
  /// - `depth`: integer >= 0, or `Infinity` for unlimited. Depth 0
  ///   returns just `{focusId}`.
  ///
  /// If `focusId` is null/undefined or not present in the payload
  /// the result is an empty Set (so navigation that points at a
  /// stale id collapses safely to "nothing visible" rather than
  /// the full graph).
  function bfsNeighborhood(payload, focusId, depth, direction) {
    var result = new Set();
    if (focusId == null) return result;
    var adj = buildAdjacency(payload);
    var dir = direction || "both";
    var maxDepth = depth === undefined || depth === null ? Infinity : depth;
    if (typeof maxDepth === "string") maxDepth = parseInt(maxDepth, 10);
    if (!isFinite(maxDepth) && maxDepth !== Infinity) maxDepth = Infinity;

    result.add(focusId);
    if (maxDepth <= 0) return result;

    var frontier = [focusId];
    for (var d = 0; d < maxDepth && frontier.length > 0; d++) {
      var next = [];
      frontier.forEach(function (id) {
        var follow = [];
        if (dir === "both" || dir === "down") {
          var outs = adj.outgoing.get(id);
          if (outs) outs.forEach(function (n) { follow.push(n); });
        }
        if (dir === "both" || dir === "up") {
          var ins = adj.incoming.get(id);
          if (ins) ins.forEach(function (n) { follow.push(n); });
        }
        follow.forEach(function (n) {
          if (!result.has(n)) {
            result.add(n);
            next.push(n);
          }
        });
      });
      frontier = next;
    }
    return result;
  }

  /// Inverse of bfsNeighborhood: returns the Set of *hidden* node
  /// ids, given the full id-set in the payload and the visible
  /// set. Convenient because the ViewState coordinator unions
  /// hidden sets across layers.
  function hiddenFromVisible(payload, visible) {
    var hidden = new Set();
    (payload.nodes || []).forEach(function (n) {
      if (!visible.has(n.id)) hidden.add(n.id);
    });
    (payload.unresolved_stubs || []).forEach(function (s) {
      if (!visible.has(s.id)) hidden.add(s.id);
    });
    return hidden;
  }

  global.ConspectusNavHelpers = {
    buildAdjacency: buildAdjacency,
    bfsNeighborhood: bfsNeighborhood,
    hiddenFromVisible: hiddenFromVisible,
  };
})(window);
