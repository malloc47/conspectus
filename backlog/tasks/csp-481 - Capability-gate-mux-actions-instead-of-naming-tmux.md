---
id: CSP-481
title: Capability-gate mux actions instead of naming tmux
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-480
ordinal: 163000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New static
  `pub const KNOWN_MUX_BACKENDS: &[&str] = &[TMUX_BACKEND]`
  in `discovery::tmux` holds the compile-time list of
  supported backend keys. `pin` parse (in
  `src/pins.rs::validate_entry`) and attach-target resolve
  (in `src/tui/actions.rs::resolve_attach_target`) both
  consult this array instead of comparing to the literal
  `"tmux"`. Error strings report the registered backend set
  (via `KNOWN_MUX_BACKENDS.join(", ")`) instead of
  hardcoding "only tmux."
  The runtime "does this backend actually support attach /
  rename / etc." check stays where it already lives — on
  the `MuxBackend` impl's `Unsupported` outcome variant.
  `resolve_attach_target`'s self-attach check remains tied
  to the tmux backend today because `RunConfig.current_tmux_session`
  is derived from `$TMUX`; CSP-483 generalizes it to
  `current_mux_session` with a backend field.
  A new backend added to `KNOWN_MUX_BACKENDS` (CSP-482's
  zellij per the docstring) picks up pin validation +
  attach dispatch automatically. Existing pin + attach
  tests pass byte-identical. All 25 suites green;
  fmt / clippy clean.
- Blockers: `CSP-480` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-009`
