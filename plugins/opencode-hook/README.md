# @conspectus/opencode-hook

OpenCode plugin that forwards session lifecycle events to `conspectus hook
write opencode` so [Conspectus](https://github.com/malloc47/conspectus) can
attribute live mux panes to the operator's current opencode session even
after in-app session switches. One package serves both opencode majors:

- **opencode 1.x** loads the package root (`dist/index.js`). See
  [ADR 0049](../../docs/adr/0049-opencode-hook-plugin-distribution.md).
- **opencode 2.x** loads the TUI entrypoint in `dist/v2/`. See
  [ADR 0126](../../docs/adr/0126-opencode-2-pane-attribution.md).

## What it does

The plugin extracts the session id, current working directory, and the
event name. The Rust writer adds process and tmux context, so the plugin
must run in the process that sits in the pane. The resulting record lands
in Conspectus's hook sidecar store and is read on every `conspectus graph`
and `conspectus tui` invocation.

On **opencode 1.x** the plugin runs inside each TUI and writes a record
whenever any of these session lifecycle events fires:

- `session.created`
- `session.updated`
- `session.status`
- `session.idle`
- `session.compacted`

On **opencode 2.x** server plugins run inside one shared background
service whose environment belongs to whichever client started it, so the
plugin ships only a TUI entrypoint, which runs inside each client. It
writes a record when the client starts showing a session (including
in-app switches, as `tui.route`) and on the 2.x equivalents of the
events above (`session.created`, `session.renamed`, `session.status`,
`session.idle`, `session.compacted`) for the session that client is
showing. Events for other sessions are ignored, since every client
receives them.

## Install

The plugin is not published to npm yet. Build it from a local checkout:

```sh
cd plugins/opencode-hook
npm install
npm run build
```

### opencode 1.x

```sh
opencode plugin "$(pwd)"
```

This tells opencode to load the plugin from your local filesystem and
updates an opencode config file accordingly. Scope depends on where you
run it: invoking it from inside a directory that already contains
`.opencode/` (or any opencode project root) writes a local-scope entry to
`<project>/.opencode/opencode.json`; otherwise it writes a user-scope
entry to `~/.config/opencode/config.json`. The install command prints the
resolved scope and path.

### opencode 2.x

opencode 2.x lists TUI plugins in `~/.config/opencode/cli.json` (or
`$XDG_CONFIG_HOME/opencode/cli.json`), separate from the server plugins
in `opencode.jsonc`. Add the built `dist/v2` directory there:

```json
{
  "plugins": ["/path/to/conspectus/plugins/opencode-hook/dist/v2"]
}
```

`opencode plugin add` accepts only npm and Git specifiers, so edit the
file directly. Restart your opencode clients afterwards. `opencode plugin
list` shows the entry once it resolves.

### Both majors

Make sure `conspectus` is on `PATH`, or set `CONSPECTUS_HOOK_BIN` to its
absolute path before starting opencode. If the binary is missing the
plugin silently no-ops (with a one-shot warning on stderr) and does not
destabilize the opencode session.

To uninstall, remove the plugin entry from the config file it was added
to and run `npm uninstall`.

## Opt out

On opencode 1.x, pass `--pure` to disable all external plugins, including
this one, for a single invocation:

```sh
opencode --pure
```

opencode 2.x has no `--pure`; remove the `cli.json` entry instead.

## Verify

After starting an opencode session, query the Conspectus hook store:

```sh
conspectus graph --format json \
  | jq '[.candidate_links[] | select(.source_metadata.adapter == "hook_sidecar" and .source.harness_key == "opencode")] | length'
```

If the count is `0` after a few seconds of activity, check stderr for
the plugin's one-shot spawn warning and confirm that `conspectus` is on
`PATH` from inside the opencode process environment. On opencode 2.x,
also confirm that `opencode plugin list` shows the `dist/v2` entry.

## Implementation notes

- Payload shape: flat `{session_id, cwd?, hook_event_name?}` JSON over
  stdin. Same contract as the existing `conspectus hook write
  claude-code` and `conspectus hook write codex` writers. Both entrypoints
  share `src/write.ts`.
- The Rust writer (`HarnessAdapter::hook_record_from_payload`) is the
  stable interface boundary; if opencode renames event variants this
  plugin updates, but the Rust side does not.
- The 2.x entrypoint has no runtime dependencies. It is typed against
  `@opencode/plugin`'s TUI API but does not import it at runtime.
- The plugin does *not* forward message, text, or tool events to keep
  sidecar churn low. The Conspectus discovery pass uses the freshest
  record per pane and does not enforce a TTL, so lifecycle and route
  events are sufficient.
- If a Conspectus record is ever written from opencode 2.x's shared
  service (for example by a server plugin of your own), the writer marks
  it as coming from a background harness and discovery never uses it for
  pane attribution (ADR 0126).
