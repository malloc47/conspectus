# @conspectus/opencode-hook

OpenCode plugin that forwards session lifecycle events to `conspectus hook
write opencode` so [Conspectus](https://github.com/malloc47/conspectus) can
attribute live mux panes to the operator's current opencode session even
after in-app `/session resume` switches. See
[ADR 0049](../../docs/adr/0049-opencode-hook-plugin-distribution.md) for
the design rationale.

## What it does

Subscribes to opencode's `event` hook and writes a hook observation
whenever any of these session lifecycle events fires:

- `session.created`
- `session.updated`
- `session.status`
- `session.idle`
- `session.compacted`

The plugin extracts the session id, current working directory, and the
event name; the Rust writer adds process and tmux context. The resulting
record lands in Conspectus's hook sidecar store and is read on every
`conspectus graph` / `conspectus tui` invocation.

## Install

The plugin is not published to npm yet. Install from a local checkout:

```sh
cd plugins/opencode-hook
npm install
npm run build
opencode plugin "$(pwd)"
```

The last command tells opencode to load this plugin from your local
filesystem and updates an opencode config file accordingly. Scope
depends on where you run it: invoking it from inside a directory that
already contains `.opencode/` (or any opencode project root) writes a
local-scope entry to `<project>/.opencode/opencode.json`; otherwise it
writes a user-scope entry to `~/.config/opencode/config.json`. The
install command prints the resolved scope and path.

Make sure `conspectus` is on `PATH`, or set `CONSPECTUS_HOOK_BIN` to its
absolute path before starting opencode. If the binary is missing the
plugin silently no-ops (with a one-shot warning on stderr) and does not
destabilize the opencode session.

To uninstall, remove the plugin entry from whichever config file
`opencode plugin` updated (local `<project>/.opencode/opencode.json`
or user-scope `~/.config/opencode/config.json`) and run `npm
uninstall`.

## Opt out

Pass `--pure` on the opencode CLI to disable all external plugins,
including this one, for a single invocation:

```sh
opencode --pure
```

## Verify

After starting an opencode session, query the Conspectus hook store:

```sh
conspectus graph --format json \
  | jq '[.candidate_links[] | select(.source_metadata.adapter == "hook_sidecar" and .source.harness_key == "opencode")] | length'
```

If the count is `0` after a few seconds of activity, check stderr for
the plugin's one-shot spawn warning and confirm that `conspectus` is on
`PATH` from inside the opencode process environment.

## Implementation notes

- Payload shape: flat `{session_id, cwd?, hook_event_name?}` JSON over
  stdin. Same contract as the existing `conspectus hook write
  claude-code` and `conspectus hook write codex` writers.
- The Rust writer (`src/hook.rs::opencode_record_from_payload`) is the
  stable interface boundary; if the opencode SDK renames event variants
  this plugin updates, but the Rust side does not.
- The plugin does *not* subscribe to `chat.message` or `tool.execute.*`
  to keep sidecar churn low. The Conspectus discovery pass uses the
  freshest record per pane and does not enforce a TTL, so lifecycle
  events alone are sufficient for the H-MUXPROC-015 drift fix.
