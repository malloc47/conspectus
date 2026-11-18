---
id: CSP-902
title: 'TUI: Preview settings and cleanup rules come from config'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-18 13:11'
updated_date: '2026-11-18 13:44'
labels:
  - tui
  - config
dependencies: []
ordinal: 978000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mux Preview's smart wrap hardcodes how it cleans an agent's screen: which characters count as decoration, how long a run of one is before it is a rule, and whether rules shorten around a label (ADR 0106, ADR 0251). An app-specific quirk, like Claude Code's right-aligned `new task? /clear to save … tokens` hint (CSP-903), would need yet more code in the same place. The operator (2026-11-18) wants this logic in configuration they can tune, with quirks as data rather than code. They chose: the generic smart-wrap settings and pattern rules together, a `[tui.preview]` section of config.toml / .conspectus.toml with the usual precedence, built-in rules named so one can be overridden or turned off by name, and an optional per-harness scope on a rule. Pattern matching needs `regex` as a direct dependency; it is already in Cargo.lock through another crate.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `[tui.preview]` sets the decoration characters, the shortest rule, and whether rules shorten around a label; unset keys keep the built-in values, and project config wins over user config, which wins over the built-ins
- [x] #2 `[[tui.preview.rules]]` entries match a line of a pane capture by regular expression and set how it fits the Preview (smart, cut, wrap, right-align, or drop); the first matching rule decides, and a rule naming a harness applies only to panes running it
- [x] #3 The built-in settings and rules are one embedded document in the config format; a rule of the same name replaces a built-in one, `enabled = false` turns one off, and a test holds the built-in settings equal to the framework defaults
- [x] #4 An invalid pattern, fit, harness, or setting is reported like other config errors and skipped; the Preview keeps working
- [x] #5 Rules apply under smart wrap only; plain and none are unchanged
- [x] #6 The framework takes the settings and per-line fits as plain values and a closure, and uses no host vocabulary
- [x] #7 An ADR records the schema, the merge, and the `regex` dependency, with a framework record; operations.md, design.md, and the richtext page describe it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. tuikit: `SmartWrap` (decoration ranges, min_rule_run, shorten_rules; Default = the old constants), `LineFit` (smart, cut, wrap, right-align, drop), and `layout_capture_with(lines, mode, source_width, width, &SmartWrap, fit)`; `layout_capture` delegates with the defaults. Fits asked under Smart only, before trailing blank lines are dropped. Tests per setting and per fit.
2. Host module `src/tui/preview_rules.rs`: `PreviewConfig` (settings + rules), `[tui.preview]` parsing with per-key diagnostics, rule merge by name (a layer's rules ahead of earlier ones, `enabled = false` removes), regex patterns, harness scope validated against the registered harness keys, `pane_harnesses` from linked sessions, contained processes, and pane/launch commands.
3. Built-in document `src/tui/preview_defaults.toml` (embedded), merged first; a test holds its settings equal to `SmartWrap::default()`.
4. Wire `TuiConfig.preview` -> `RunConfig.preview` -> `format_preview_for_mux`.
5. `regex` as a direct dependency (already in the lock).
6. ADR 0252 (framework-owned in part), framework record 0072, indexes, ADR 0106/0251 status, record 0058, richtext page, architecture.md, operations.md section, design.md, CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Validation: `just check` passes (5965 tests, tuikit-features, clippy, rustdoc, diff check). Lockfile gains only the `regex` edge on conspectus; no package added. `pane_harnesses` reads the snapshot by reference, one pass over the links per frame, only when rules exist.
Deviation from the plan shown to the operator: the `/clear` rule ships in the same commit as the mechanism, since the built-in document and its tests name it; CSP-903 is closed alongside.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Preview's smart-wrap cleanup is configuration. `[tui.preview]` sets `decoration` (characters and `a..b` ranges), `min_rule_run`, and `shorten_rules`; `[[tui.preview.rules]]` entries (`name`, `match` regex, `fit` of smart/cut/wrap/right-align/drop, optional `harness`, `enabled`) fit matching lines of a pane capture, the first applicable match deciding, under smart wrap only. Built-in settings and rules are an embedded document in the same format (`src/tui/preview_defaults.toml`), merged before the user's and project's config: settings key by key, rules by name with a layer's rules checked first and `enabled = false` removing one. Bad values and rules are config diagnostics and are skipped; a rule with an unknown key is skipped so a misspelled scope cannot widen it. A pane's harnesses come from linked sessions, contained agent processes, and its pane or launch command. The framework gained `SmartWrap`, `LineFit`, and `layout_capture_with` (values and a closure; `layout_capture` keeps the defaults). `regex` is a direct dependency. ADR 0252 (framework-owned in part) and framework record 0072; ADR 0106, 0251, record 0058, the richtext page, architecture.md, operations.md (new `[tui.preview]` section), design.md, and CHANGELOG updated. Verified by 9 framework tests (each setting, each fit, smart-only, rows within the area), 16 rule tests (built-in document equals the framework defaults, merge, scope, diagnostics, pane harnesses), 2 config-loader tests, 3 Preview tests, `just check`, and a snapshot of a live Claude Code capture.
<!-- SECTION:FINAL_SUMMARY:END -->
