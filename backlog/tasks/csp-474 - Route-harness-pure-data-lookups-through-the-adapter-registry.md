---
id: CSP-474
title: Route harness pure-data lookups through the adapter registry
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-473
ordinal: 156000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. `HarnessAdapter` grows two methods
  (`display_label()` defaulting to the harness key,
  `launch_options()` defaulting to empty); `harness_key`
  tightens to `&'static str` to match every production impl.
  New module-level static `REGISTERED_ADAPTERS: LazyLock<Vec<
  Box<dyn HarnessAdapter>>>` holds the canonical 4-adapter set
  in the pre-H-EXT-002 UI order (claude-code first) so the
  TUI filter menu stays byte-identical. `ClaudeCodeAdapter`
  gets `display_label = "claude"` (CSP-154 collapse);
  `CodexAdapter` + `ClaudeCodeAdapter` override
  `launch_options` to expose their skip-permissions
  fragments.
  Six parallel-table functions rewired to iterate the
  registry: `launch_argv_for`, `resume_argv_for`,
  `launch_options_for`, `strip_known_launch_option_fragments`
  (harness/mod.rs); `harness_label` (rows/mod.rs) delegates
  to a new `harness::display_label_for`; `resolve_resume_target`
  (tui/resume.rs) delegates to `resume_argv_for` and joins
  the argv into the operator-visible command string.
  `HARNESS_OPTIONS` const → `harness_options()` function
  returning `&'static [&'static str]` derived from
  `harness_keys()`. Seven registry anchor tests cover key
  ordering, label collapse, launch-option overrides,
  resume-argv dispatch, and the strip-fragments walk.
  All 25 suites (1514 lib tests) pass; fmt / clippy clean.
- Blockers: `CSP-473` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-002`
