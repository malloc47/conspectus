---
id: CSP-049
title: Add config loading for session projection defaults
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies: []
ordinal: 47000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: write an ADR for the Conspectus config file layout (project
  `.conspectus.toml` first, then `$XDG_CONFIG_HOME/conspectus/config.toml`
  or `$HOME/.config/conspectus/config.toml`, plus precedence and
  schema), then add a `config` module that loads
  `[session] projection = "agent" | "mux" | "union"`. Update
  `docs/design.md` if config introduces new model requirements.
- Tests: unit tests for default projection, project-local override,
  user-level override, invalid projection values, malformed TOML, and
  missing config files.
- Manual checks: verify the loader is read-only.
- Blockers: ADR for config layout (filed alongside this item).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0012 records the config layout, precedence, and the
minimal `[session] projection` schema. New `src/config.rs` module
exposes `Config`, `SessionConfig`, `Projection`, and a
`ConfigLoader` that takes explicit `$HOME` / `$XDG_CONFIG_HOME`
so tests don't mutate process state. Project config walks upward
from cwd and stops at the `$HOME` boundary. Missing files are
not errors; malformed TOML and invalid `projection` values emit
`ConfigDiagnostic`s and fall back to defaults. Ten new unit
tests cover defaults, project / user overrides, project >
user precedence, the home-boundary stop, invalid value, malformed
TOML, unknown-keys-ignored, `Projection::parse`/`as_str` round
trip, and `$XDG_CONFIG_HOME` overriding `$HOME/.config`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-005`
