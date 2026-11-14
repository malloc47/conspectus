# ADR 0230: `hook status opencode` Reads Where opencode's Config Lists The Plugin

## Status

Accepted. Extends `conspectus hook status` (ADR 0206 decision 6, the
health line) to opencode, whose hook is a plugin installed by hand
([ADR 0049](0049-opencode-hook-plugin-distribution.md), with the 2.x
listings of [ADR 0126](0126-opencode-2-pane-attribution.md) and
[ADR 0229](0229-an-opencode-2-client-reports-the-sessions-it-shows-when-no-server-plugin-does.md)).
Reads opencode's plugin config files; adds no program, subcommand of
another program, socket, signal, or write path (ADR 0117, ADR 0143), and
no dependency.

## Context

`conspectus hook status` took only claude-code, codex, and pi, so the
live-state health line (receiving, silent, no-events, off) was not
available for opencode, whose plugin reports live state; the operations
guide pointed operators at `table sessions --columns +live` instead. The
store's entries alone can say `receiving`. Saying installed or off means
reading opencode's own config, where the operator lists the plugin.
There is still no `hook init opencode` (research note 03, open question
8): writing opencode's JSONC config while keeping its comments would need
an editor the repository does not have.

Where opencode lists plugins (research note 03, section 5.1; ADR 0125):

- 1.x: the `plugin` array of `opencode.json` (the plugin's README also
  names `config.json`, which `opencode plugin` was said to write), in
  `$XDG_CONFIG_HOME/opencode/` or a project's `.opencode/`.
- 2.x: the `plugins` array of `opencode.jsonc` in
  `$XDG_CONFIG_HOME/opencode/`, the project root, or `.opencode/`, whose
  packages load as server plugins and in every client; and the `plugins`
  array of `$XDG_CONFIG_HOME/opencode/cli.json` (user scope only), whose
  packages load in each client alone.

## Decision

1. **`hook status opencode` takes the usual scopes.** The user scope
   reads `opencode.json`, `opencode.jsonc`, `config.json`, and
   `cli.json` under `$XDG_CONFIG_HOME/opencode` (`~/.config/opencode`
   without it). The project scope reads `opencode.json` and
   `opencode.jsonc` in the current directory and in its `.opencode/`.
   Nothing else of opencode's is read: no state, no data, no other
   config file, no environment override. `hook init opencode` and
   `hook remove opencode` still do not exist; the value is accepted by
   `status` alone.

2. **JSONC without a dependency.** Each file is read with its comments
   (`//` and `/* */`, outside strings) and trailing commas dropped, then
   parsed as JSON. A file that still does not parse as an object is
   reported `unreadable`, with the parser's message on stderr, and counts
   for nothing; status still succeeds.

3. **What counts as the plugin.** An entry of `plugin` or `plugins`, a
   string or a `{"package": ...}` object, is the Conspectus plugin when
   it is a path with a component `opencode-hook` (the checkout's
   `plugins/opencode-hook`, its root or `dist/v2`) or the package name
   `@conspectus/opencode-hook`. A bare package name, or another scope's,
   is some other plugin.

4. **The lines.** First `<installed|not-installed>\topencode\t<dir>` for
   the scope, then one line per file that exists:
   `installed\topencode\t<path>\t<loads>`, where `<loads>` is `1.x` (a
   `plugin` entry), `2.x` (a `plugins` entry of a config file: the
   server entrypoint and the client's), or `2.x client only` (a
   `cli.json` entry); `not-installed\topencode\t<path>`; or
   `unreadable\topencode\t<path>`. A listing only in `cli.json` adds a
   stderr note that each client reports only the sessions it has shown,
   and one in both `cli.json` and a config file a note that each client
   loads its entrypoint twice. Then the health line, as for every
   harness.

5. **The health line.** It counts the plugin installed when any file in
   the scope lists it, whatever it loads, since every listing reports
   live state (ADR 0229 for `cli.json`). Its `off` message, which names a
   `hook init` command for the other harnesses, says to build the
   plugin and list it in opencode's config instead. Status cannot tell
   which version of the plugin a listing points at, so a plugin built
   before live state reads `silent` once an agent runs.

## Consequences

- An operator can check an opencode install the way they check the
  others, and troubleshoot it from the same health labels.
- The listing is read as opencode is documented to read it; an install
  through a file status does not read (an environment override, a
  plugins directory) reads `not-installed`, though the health line still
  says `receiving` once entries arrive.
- `tests/capabilities.rs` traces `hook status opencode` reading a seeded
  `cli.json` and writing nothing.

## Alternatives Considered

- **A JSONC crate.** Would also allow a future `hook init opencode` that
  keeps comments, but reading needs only comments and trailing commas
  dropped, a few dozen lines. A crate waits for the install command and
  its own ADR.
- **Match any entry containing `opencode-hook`.** A third-party
  `my-opencode-hook` or `@someone/opencode-hook` would count. Rejected
  for path components and the package's own scope.
- **Read the plugin's build (`package.json` beside the listed path) for
  its version.** A file outside opencode's config, in the operator's
  checkout, for a check the health line already approximates. Rejected.

## Open Questions Answered

- *Support `hook init opencode`?* Not here: status only reads. Research
  note 03's question 8 stays open.
