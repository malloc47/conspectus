---
id: CSP-223
title: Audit harness hooks/plugins as definitive session-state sidecar emitters
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 260000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: determine whether supported harnesses can expose current
  session state through lifecycle hooks, tool hooks, plugins, or
  status-line callbacks without sending text into the agent
  conversation. Prototype one hook/plugin per harness that writes
  its raw payload and selected environment/process context to a
  temporary Conspectus-owned sidecar directory. Audit Claude Code
  hooks, Codex `codex_hooks`, opencode plugin/server extension
  points, and aider if a native hook or extension surface exists.
  For each harness, record whether the payload includes an explicit
  session id, transcript/session path, cwd, pid/ppid, tmux pane,
  and event timestamp; also verify whether hook execution changes
  transcript/session logs. Do not add a persistent Conspectus hook
  convention until the audit proves at least one harness can emit
  useful non-mutating state.
- Tests: none for the audit itself. Keep any prototype scripts out
  of committed config unless they become intentional fixtures.
- Manual checks: run a short live session per harness with the
  prototype hook enabled; diff the harness transcript/state before
  and after to confirm only expected harness activity changed and
  no Conspectus probe text was logged.
- Blockers: none. Follow-up ADR required before adding a durable
  sidecar schema or installer.
- **audit slice landed (Claude)**: Claude Code hooks are viable
  and provide `session_id`, `transcript_path`, `cwd`, and event
  name on stdin; `SessionStart` covers startup, resume, clear,
  and compact. ADR 0028 records the sidecar path.
- **audit slice landed (opencode, 2026-05-30)**: OpenCode hooks
  are viable. The plugin API ships as `@opencode-ai/plugin`
  (npm), installed via `opencode plugin <module>` into
  `$XDG_CONFIG_HOME/opencode/node_modules` and listed in
  `config.json#plugin`. A plugin is a TypeScript module with
  default export `(input: PluginInput) => Promise<Hooks>`. The
  `event` hook receives the full `Event` union from
  `@opencode-ai/sdk`; relevant variants include
  `EventSessionCreated` and `EventSessionUpdated` (both carry
  the full `Session` object: `id`, `directory`, `parentID`,
  `title`, `time.{created,updated}`, `share.url`),
  `EventSessionStatus` and `EventSessionIdle` and
  `EventSessionCompacted` (carry `sessionID`), and `chat.message`
  / `chat.params` / `chat.headers` / `tool.execute.{before,after}`
  / `command.execute.before` / `permission.ask` (all carry
  `sessionID`). `PluginInput` exposes `project`, `directory`,
  `worktree`, `serverUrl`, `client`, and a `BunShell`; the
  plugin runs in-process so `process.pid`, `process.ppid`, and
  `process.env.TMUX{,_PANE,_TMPDIR}` are directly readable for
  pane/process context. The hook write path is non-mutating —
  forwarding events to `conspectus hook write` does not append
  to the opencode session DB, transcript, or HTTP API. Opt-out
  exists at the CLI level via `opencode --pure`. Implementation
  plan for `CSP-229` is therefore well-scoped: ship a
  `@conspectus/opencode-hook` npm plugin that calls a new
  `conspectus hook write opencode` writer (sibling of the
  existing `claude-code` and `codex` writers), and reuse the
  existing `discovery::hook_sidecar` post-merge pass which is
  already harness-agnostic.
- **audit slice landed (codex, 2026-05-30)**: Codex has a hook
  surface (the top-level CLI flag `--dangerously-bypass-hook-trust`
  proves it, with `codex plugin` for marketplace plugins). The
  exact event names, payload shapes, and config schema were not
  extractable from `codex --help` or from a strings dump of the
  wrapped binary on this machine and would need upstream docs
  or source reading. Lower priority than opencode because
  `CSP-218` / ADR 0048 already closes the codex side of
  the CSP-227 drift class via log-derived attribution; the
  `conspectus hook write codex` writer subcommand exists as a
  stub for future use if/when codex hook payload semantics are
  documented or reverse-engineered.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-009`
