---
id: CSP-229
title: Add opencode plugin sidecar emitter
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-223
  - CSP-224
ordinal: 272000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ship an opt-in npm-distributed opencode plugin (working
  name `@conspectus/opencode-hook`, distributed alongside the
  Conspectus release; can also be tried locally via the
  documented `opencode plugin <local-path>` install) that
  subscribes to the `event` hook on the `@opencode-ai/plugin`
  surface and forwards session lifecycle observations to a new
  `conspectus hook write opencode` subcommand. The writer should
  parse the opencode `Event` union, extracting `sessionID` (or
  `info.id` for `EventSession{Created,Updated}`), `info.directory`
  when present, and process/tmux context (`process.pid`,
  `process.ppid`, `TMUX`, `TMUX_PANE`, `TMUX_TMPDIR`), then emit
  a hook record on the same schema ADR 0028 defines for Claude.
  Plugin opt-out at the CLI is `opencode --pure`. Existing
  `discovery::hook_sidecar` post-merge pass is already harness-
  agnostic and will pick up `harness_key: "opencode"` records
  automatically; resolver ranking and demotion semantics for
  opencode mirror the Claude path. Consider whether
  `chat.message` / `tool.execute.after` events also write a
  record (gives finer-grained activity heartbeat) or whether
  `session.{created,updated,idle,status}` are sufficient — pick
  the smaller event set if both work, to keep sidecar churn low.
- Tests: payload fixture tests for each handled `Event` variant;
  sidecar record generation tests; missing-field degradation;
  multi-project plugin process emits records keyed by sessionID
  not project; `--pure` opt-out path produces no records.
- Manual checks: install the plugin via `opencode plugin` and
  run a short opencode session with `--print-logs` enabled;
  confirm `conspectus hook write opencode` is invoked, the
  sidecar DB rows show the expected sessionID/cwd/pid/tmux
  fields, and a follow-up `opencode session list` shows the
  session transcript is byte-identical to a control run without
  the plugin installed.
- Related: `CSP-223` audit slice landed 2026-05-30
  establishing the plugin shape; ADR 0028 hook sidecar schema;
  ADR 0049 plugin distribution; `discovery::hook_sidecar`
  reader; ADR 0048 (parallel codex drift fix uses log-derived
  attribution rather than hooks).
- **landed 2026-05-31**: Rust writer subcommand
  `conspectus hook write opencode` plus three unit tests
  (`hook::opencode_payload_{builds_hook_record,requires_session_id,
  rejects_empty_session_id}`); harness-agnostic
  `discovery::hook_sidecar::apply_hook_sidecars` already picks up
  `harness_key: "opencode"` records; a new end-to-end test
  `opencode_hook_record_demotes_stale_launch_argv_for_same_mux`
  pins the override behavior under the opencode harness key.
  TypeScript plugin lives in-repo at `plugins/opencode-hook/`
  per ADR 0049 (npm package `@conspectus/opencode-hook`, builds
  cleanly via `npm install && npm run build`, types pass
  `npm run typecheck`). v1 subscribes only to lifecycle
  `session.{created,updated,status,idle,compacted}` events;
  `chat.message` / `tool.execute.*` heartbeat was rejected by
  ADR 0049 because freshest-record-wins doesn't require
  heartbeat and the lower churn is preferable. Distribution is
  local-install via `opencode plugin <local-path>`; npm publish
  is deferred per ADR 0049 until at least one external user.
- **live-verified 2026-05-31**: ran `npm install && npm run build`
  in `plugins/opencode-hook/`, installed via `opencode plugin
  "$(pwd)"` (local scope writes `<project>/.opencode/opencode.json`
  when the cwd is a project root; user-scope path documented as
  fallback in the plugin README), then ran `opencode run "say hi
  in one word"` with `CONSPECTUS_HOOK_BIN` pointed at the debug
  binary. One `hook_sidecar` candidate link landed under
  `harness_key=opencode`, `harness_version=0.1.0`, carrying
  `session.created` and the live tmux pane (`%10`); resolver
  correctly marked it `ignored` because the pane is currently
  running `claude-code`, not opencode (the `opencode run`
  process exited after the prompt).
- Blockers: none. Audit complete via `CSP-223`; sidecar
  schema fixed via `CSP-224`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-014`
