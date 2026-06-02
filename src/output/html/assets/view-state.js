// view-state.js — coordinator that owns the multiple layers of
// "hide this id" wishes that chrome modules push down to the
// GraphDriver. The filter panel sets one layer (kinds / relations
// / state / candidate-mode toggles). The navigation chrome sets a
// second layer (depth/direction restriction around the focused
// node). The driver only sees the union.
//
// Why this exists: GraphDriver.setHidden takes ONE id-set. Without
// a coordinator the most recent caller would clobber the other.
// A small layered coordinator lets both chrome modules stay
// independent while composing correctly.

(function (global) {
  "use strict";

  function ConspectusViewState(driver) {
    this.driver = driver;
    this.layers = {
      filter: new Set(),
      nav: new Set(),
    };
  }

  ConspectusViewState.prototype.updateFilter = function (hiddenIds) {
    this.layers.filter = normalizeSet(hiddenIds);
    this._push();
  };

  ConspectusViewState.prototype.updateNav = function (hiddenIds) {
    this.layers.nav = normalizeSet(hiddenIds);
    this._push();
  };

  ConspectusViewState.prototype.clearNav = function () {
    this.layers.nav = new Set();
    this._push();
  };

  ConspectusViewState.prototype.snapshot = function () {
    return {
      filter: new Set(this.layers.filter),
      nav: new Set(this.layers.nav),
    };
  };

  ConspectusViewState.prototype._push = function () {
    var union = new Set(this.layers.filter);
    this.layers.nav.forEach(function (id) { union.add(id); });
    this.driver.setHidden(union);
  };

  function normalizeSet(input) {
    if (!input) return new Set();
    return input instanceof Set ? input : new Set(Array.from(input));
  }

  global.ConspectusViewState = ConspectusViewState;
})(window);
