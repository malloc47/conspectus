---
id: CSP-144
title: Add server status and inspection subcommands
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 384000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: 5 integration tests in `tests/cli_serve.rs` —
  status (no-daemon + daemon-present), per-class refresh
  (unknown class error, in-process fallback, daemon-routed).
- Deferred: `--reload-config` for picking up TOML changes
  without restarting. Wants a small ADR for the SIGHUP
  semantics (ADR 0080 already pre-positioned for the signal
  layer) and a config-diff strategy for intervals that
  change mid-run. Landing it stand-alone is cleaner than
  bundling here.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed alongside the CSP-142 daemon work.
- The daemon now tracks a per-class `SchedulerState`
  (`last_started_epoch`, `last_completed_epoch`,
  `last_outcome` of `"ok"`/`"error"`, `last_error`
  message). Each class thread writes its entry on cycle
  start + cycle completion; the `status` socket command
  snapshots the map under the Mutex (briefly held) and
  returns it as JSON.
- `conspectus status` is the CLI client. `--format human`
  (default) prints one line per class with last-tick
  freshness; `--format json` emits the structured shape
  operators / monitoring can consume. ADR 0038's
  "absence is not an error" guarantee holds — no daemon
  prints "no daemon running" + exits 0.
- `conspectus refresh --class <name>` (or socket arg
  `{"command": "refresh", "args": {"class": "<name>"}}`)
  forces only one provider class to re-run. The daemon
  side reuses the same `try_class_cycle` the scheduler
  runs; the in-process fallback mirrors that flow so the
  semantic is identical with or without a daemon up.
  `ProviderClass::parse` is the operator-string → enum
  bridge.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-008`
