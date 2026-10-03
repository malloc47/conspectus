---
id: CSP-479
title: Generalize the codex_log-style aux-reader wiring
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-473
  - CSP-476
ordinal: 161000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. `HarnessAdapter` gains
  `apply_aux_attribution(snapshot, ctx: &AuxAttributionContext)`
  with a `{ }` no-op default. The context packages the harness
  state root, the per-mux `(harness_key, pid)` set from
  `active_harness_pids_per_mux`, and the `now_epoch` — the same
  inputs the pre-H-EXT-007 hardcoded codex-log branch consumed.
  Codex adapter overrides `apply_aux_attribution` to read its
  own `CONSPECTUS_CODEX_LOG_WINDOW_SECONDS` env var and
  delegate to `codex_log::apply_codex_log_attribution`. The
  codex-log knob now lives in one place (the adapter) instead
  of split across `LocalDiscoveryConfig` + `from_env` + the
  hardcoded branch.
  `LocalDiscoveryConfig` sheds two codex-specific fields
  (`codex_log_disabled`, `codex_log_window_seconds`) and their
  two builders (`without_codex_log`, `with_codex_log_window`,
  the latter unused). Replaced with a general
  `disabled_aux_harnesses: BTreeSet<String>` set + a
  `without_aux_harness(key)` builder; `CONSPECTUS_DISABLE_CODEX_LOG`
  still populates the set with the codex key for wire
  compatibility with operator env configs.
  `apply_mutators` iterates `harness::registered_adapters()`,
  skips disabled entries, builds an `AuxAttributionContext`
  per adapter, and calls `apply_aux_attribution`. A fifth
  harness with an aux surface adds only its trait impl —
  no `LocalDiscoveryConfig` field, no named branch at the
  caller, no ADR-shape drift.
  `dev_scenarios` migrates from `.without_codex_log()` to
  `.without_aux_harness(codex::HARNESS_KEY)`. ADR 0048 updated
  to record that the `with_codex_log_window` builder was
  retired in favor of the env-var-only override.
  All 25 suites (1516 lib tests) pass byte-identically; fmt /
  clippy clean.
- Blockers: `CSP-473` (landed), `CSP-476` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-007`
