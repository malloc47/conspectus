# Pi hook extension

A [Pi](https://pi.dev) extension that reports session switches to
`conspectus hook write pi`, so
[Conspectus](https://github.com/malloc47/conspectus) can tell which session
a tmux pane is running even after `/new`, `/resume`, `/fork`, or `/clone`
inside one Pi process. Pi doesn't keep its session file open, so without
the extension Conspectus can only guess from the command line and cwd.
[ADR 0121](../../docs/adr/0121-pi-harness-support.md) records the design.

## What it does

On every `session_start` (reasons `startup`, `reload`, `new`, `resume`,
and `fork`) in the interactive TUI, the extension runs
`conspectus hook write pi` with:

```json
{
  "session_id": "<id>",
  "cwd": "<session cwd>",
  "transcript_path": "<session file>",
  "hook_event_name": "session_start:<reason>"
}
```

Conspectus adds the process and tmux context and stores the record in its
hook sidecar directory. The payload carries no message content.

It writes nothing for `--no-session` runs, which have no session file,
or in Pi's print, JSON, and RPC modes, which don't run in a pane.

## Install

`conspectus` carries this file and installs it:

```sh
conspectus hook init pi                   # ~/.pi/agent/extensions/conspectus-hook.ts
conspectus hook init pi --scope project   # ./.pi/extensions/conspectus-hook.ts
```

The user scope follows `PI_CODING_AGENT_DIR` when it is set. Pi asks you
to trust a project before it loads project extensions. Restart Pi, or
run `/reload`, to load the extension.

`conspectus hook status pi` reports `installed`, `outdated` (an older
copy; run `hook init pi` again to update it), `not-installed`, or
`foreign` (a file at that path that Conspectus didn't write, which
`hook init` and `hook remove` leave alone).

No build step or package is needed. Pi loads TypeScript extensions
directly.

## Binary lookup

The extension runs `conspectus` from `PATH`. Set `CONSPECTUS_HOOK_BIN` to
an absolute path before starting Pi to use a different binary. If the
binary is missing or the write fails, the extension does nothing and Pi
carries on.

## Opt out

```sh
conspectus hook remove pi                   # or --scope project
```

This deletes only the file Conspectus installed. Other extensions in the
directory are untouched. Pi's own extension controls can also disable it
for a run.
