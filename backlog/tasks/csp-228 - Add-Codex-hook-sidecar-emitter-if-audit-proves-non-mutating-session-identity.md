---
id: CSP-228
title: Add Codex hook sidecar emitter if audit proves non-mutating session identity
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-223
  - CSP-224
ordinal: 271000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: if Codex `codex_hooks` events include the active
  thread/rollout/session id, or if a hook can reliably identify the
  current rollout path from its process context, provide an opt-in
  hook emitter for Conspectus sidecar records. Avoid depending on
  terminal input, prompt text, or transcript mutation. If Codex
  hooks only run for tool events, document the expected delay
  before the first sidecar record appears in a fresh session.
- Tests: payload fixture tests for supported Codex hook events;
  sidecar generation tests for explicit session id and transcript
  path cases; stale/missing-field degradation.
- Manual checks: enable the hook for a live Codex session and
  confirm Conspectus links the active rollout even when the launch
  command names only a resumed parent.
- Blockers: `CSP-223`, `CSP-224`.
- Audit notes:
  - Local Codex 0.128.0 already gives strong non-mutating live
    evidence through tmux active pane pid -> `/proc/<pid>/fd` ->
    open rollout JSONL path. In live testing, the process argv named
    an older resumed thread, while the open fd identified the current
    rollout, and Conspectus emitted
    `active_pane_fd_session_match` for the current `codex`
    `AgentSession`.
  - Generated app-server schemas expose hook events
    `sessionStart`, `userPromptSubmit`, `postToolUse`, `preToolUse`,
    `permissionRequest`, and `stop`, plus hook notifications with
    `threadId`; this is useful control-plane evidence but not enough
    by itself to identify the currently active thread inside an
    arbitrary already-running TUI process.
  - Codex user hooks are accepted in `$CODEX_HOME/config.toml` under
    `[hooks]` with PascalCase event keys such as `SessionStart`.
    `hooks/list` reports them as `eventName = "sessionStart"`.
    Command hooks must currently be synchronous; `async = true` is
    parsed but skipped with a warning.
  - A `SessionStart` command hook invoked by `codex exec` receives
    JSON on stdin containing `session_id`, `transcript_path`, `cwd`,
    `hook_event_name`, `model`, `permission_mode`, and `source`.
    Ephemeral runs can have `transcript_path = null`; persisted runs
    include the rollout JSONL path.
  - Hook command environments include `TMUX` and `TMUX_PANE`, which
    are enough to correlate the hook event back to a mux pane. Do
    not trust inherited `CODEX_THREAD_ID` for attribution; live
    probing showed it can name the parent Codex session that launched
    the probe rather than the hook payload's new `session_id`.
- Recommended implementation path: first harden and test the
  existing active-pane fd Codex signal as a default, no-opt-in
  current-session source; then add `conspectus hook write codex`
  for `SessionStart` payloads and `conspectus hook init codex`
  editing `$CODEX_HOME/config.toml` as the opt-in durable path.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `conspectus hook write codex`, which converts
Codex `SessionStart` hook JSON into schema-v1 SQLite sidecar
records with `harness_key = "codex"`. Added
`conspectus hook init/status/remove codex`, which manages a
synchronous `SessionStart` command hook in `$CODEX_HOME/config.toml`
or `~/.codex/config.toml` while preserving unrelated TOML config.
Hook-sidecar discovery now infers Codex state scope from the
`.codex` ancestor in rollout paths when it must synthesize a
sparse session node. Active-pane fd evidence remains the default
no-opt-in current-session source for already-running Codex TUI
processes.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-013`
