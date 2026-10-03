# ADR 0049: OpenCode Hook Plugin Distribution

## Status

Accepted

## Context

`CSP-229` ships an opencode plugin that subscribes to lifecycle
events and forwards them to `conspectus hook write opencode`. The audit
under `CSP-223` established that opencode's plugin surface is
exclusively the `@opencode-ai/plugin` npm package: plugins are TypeScript
modules with `default export (PluginInput) => Promise<Hooks>`, installed
via `opencode plugin <module>` into `$XDG_CONFIG_HOME/opencode/
node_modules` and listed in `config.json#plugin`. Unlike Claude Code,
which accepts any executable command as a hook, opencode will only load
JavaScript/TypeScript that conforms to the SDK shape.

Conspectus is a Rust project. Adding a TypeScript/npm artifact introduces
a new language ecosystem and distribution path. CLAUDE.md requires
documenting new persistent conventions before adopting them. The decision
points are:

- Source location.
- Distribution path (npm registry, GitHub release, source-only).
- Dependency on Conspectus binary discovery from inside Node.
- TypeScript build tooling and CI integration.

A relevant precedent exists: `scripts/conspectus-claude-hook-sidecar.py`
is a Python helper that ships in-repo and is referenced by
`conspectus hook init claude-code`. The plugin will follow the same
"companion artifact in the conspectus repo" model where reasonable.

## Decision

The opencode plugin lives in-repo under
`plugins/opencode-hook/` as a normal npm package whose source is the
authoritative artifact. Boundaries:

1. **In-repo source.** `plugins/opencode-hook/{package.json,
   tsconfig.json, src/index.ts, README.md}` are committed. The package
   name is `@conspectus/opencode-hook`. Build output (`dist/`,
   `node_modules/`) is gitignored.
2. **No CI build initially.** The Rust test suite continues to be the
   only mandatory CI surface. The plugin is small enough that
   compilation correctness is checked by reading the source and by users
   who install it; if/when the plugin grows or breakage becomes a
   recurring problem, a separate `nix develop -c npm run build` CI step
   is reasonable and can be added without an ADR.
3. **No initial npm publish.** Users install by pointing `opencode
   plugin` at a local path or by cloning the repo and running
   `npm install && npm run build` under `plugins/opencode-hook/`.
   Publishing to npm is a separate operational decision and would
   benefit from at least one external user before it lands.
4. **Conspectus binary discovery.** The plugin spawns `conspectus hook
   write opencode` via `child_process.spawn`. The binary path is taken
   from `process.env.CONSPECTUS_HOOK_BIN` if set, otherwise the literal
   string `"conspectus"` is used and `PATH` resolution applies. When
   the binary is missing, the spawn error is caught and the plugin
   silently no-ops so opencode is not destabilized; a one-shot warning
   may be printed to stderr on first failure.
5. **Event selection.** The plugin subscribes to `event` only, filtering
   to the session lifecycle variants:
   `session.{created,updated,status,idle,compacted}`. It does not
   subscribe to `session.deleted` (a deletion does not represent the
   currently-active session and would skew attribution briefly), to
   `chat.message` (constant churn during a turn), or to
   `tool.execute.*` (similar churn). The lifecycle variants are
   sufficient because the existing
   `discovery::hook_sidecar::apply_hook_sidecars` post-merge pass uses
   the freshest record per pane regardless of absolute age, so periodic
   heartbeat records are not required for the CSP-227-style
   drift fix to work.
6. **Payload shape contract.** The plugin normalizes the SDK `Event`
   union into the flat payload `{session_id, cwd?, hook_event_name?}`
   already accepted by the Rust writer. `session_id` is sourced from
   `event.properties.info.id` for `session.{created,updated}` and from
   `event.properties.sessionID` for `session.{status,idle,compacted}`.
   `cwd` is sourced from `event.properties.info.directory` when
   present, falling back to `input.directory` or `input.worktree`.
   `hook_event_name` is the event type string, e.g. `"session.idle"`.
7. **Process and tmux context.** Captured by the Rust writer itself
   from `process.id()`, `parent_pid()`, and `tmux_context()` — the
   plugin does not need to pass these. The writer also accepts an
   optional `harness_version` via the `CONSPECTUS_OPENCODE_HOOK_VERSION`
   env var; the plugin sets it from `package.json#version` so records
   carry the plugin version that produced them.
8. **CLI opt-out.** `opencode --pure` already disables external
   plugins, so no Conspectus-side opt-out is required at the plugin
   level. The Rust side already supports `CONSPECTUS_DISABLE_CODEX_LOG`
   for the codex linker; an equivalent opencode disable knob is not
   added because the hook sidecar reader is already harness-agnostic
   and naturally degrades when no records exist.

## Consequences

- OpenCode panes that have been resumed in-app to a different session
  than their launch argv get correct attribution the same way Claude
  panes do today, without requiring the user to install or run anything
  extra at runtime once the plugin is in place.
- The Conspectus repo gains TypeScript/npm source. Contributors who
  only work on the Rust side never need to touch it; the plugin builds
  and runs independently.
- Discovery code stays untouched. The existing
  `discovery::hook_sidecar::apply_hook_sidecars` post-merge pass picks
  up `harness_key: "opencode"` records automatically and applies the
  same demotion rules to stale `active_pane_command_session_match`
  candidates that Claude already enjoys.
- The plugin's `event`-only subscription model means the freshest
  record for a pane is at most one lifecycle event old. For a long-
  idle opencode session this might be hours; that is acceptable
  because the freshest-record-wins discovery rule does not enforce a
  TTL on hook records during attribution.
- If `@opencode-ai/sdk` renames or restructures the session event
  variants, the plugin needs an update but the Rust writer does not.
- Future publishing to npm or addition of a CI build is a non-breaking
  change and does not require revisiting this ADR.

## Alternatives Considered

- **Ship the plugin out-of-tree.** Rejected for v1 because the writer
  and plugin co-evolve closely; an out-of-tree plugin makes it easy
  for the two to drift, and the precedent of
  `scripts/conspectus-claude-hook-sidecar.py` favors in-tree.
- **Publish to npm immediately.** Deferred. The plugin needs at least
  one external user before npm-publish overhead is justified. Local
  install via `opencode plugin <local-path>` is sufficient until then.
- **Add a CI build for the plugin.** Deferred. Source is small enough
  to review by inspection; if the plugin grows or breaks repeatedly,
  CI can be added.
- **Subscribe to `chat.message` for heartbeat records.** Rejected for
  v1 because the freshest-record-wins discovery rule does not require
  heartbeat — the lifecycle events are sufficient for the drift fix.
  If subsequent freshness-based filtering is added to
  `apply_hook_sidecars`, this decision can be revisited without
  changing the writer.
- **Have the writer accept opencode's raw `Event` JSON directly.**
  Rejected because it would couple the Rust writer to the opencode
  SDK shape and force a Rust release every time opencode adds an
  event variant. The flat normalized payload keeps the writer
  shape-agnostic.
- **Use the opencode HTTP server (`opencode serve`) instead of a
  plugin.** Rejected because the server requires an explicit launch
  mode that most users do not run, while the plugin loads
  transparently into the default TUI.
- **Add a `CONSPECTUS_DISABLE_OPENCODE_HOOK` env knob.** Deferred.
  `opencode --pure` already disables the plugin at the opencode side,
  and the Rust reader degrades silently when no records exist. If a
  use case for "plugin installed but hook reads suppressed" surfaces,
  a knob can be added without an ADR.

## Open Questions Answered

- Plugin source lives in `plugins/opencode-hook/` and is the
  authoritative artifact; `dist/` is generated and gitignored.
- The plugin is not published to npm in v1; users install locally.
- The Conspectus binary path is `process.env.CONSPECTUS_HOOK_BIN` or
  the literal `"conspectus"` via `PATH`.
- Missing binary, broken spawn, and writer errors silently no-op so
  opencode is never destabilized.
- The plugin subscribes only to lifecycle `session.*` events; no
  `chat.message` or `tool.execute.*` heartbeat in v1.
- The Rust writer's flat payload contract is the API boundary between
  the plugin and Conspectus; the writer stays opencode-SDK-shape-
  agnostic.
