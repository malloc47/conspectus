# ADR 0240: Hook Commands Run A `conspectus` Path That Outlives An Upgrade

## Status

Accepted. Changes the command `conspectus hook init claude-code|codex`
installs ([ADR 0028](0028-hook-sidecar-mux-attribution.md), with the
per-event entries of
[ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md))
and what `hook status` reports. Reads files and the environment only:
no new program, subcommand of another program, socket, signal, write
path, or dependency (ADR 0117, ADR 0143).

## Context

`hook init` wrote the running binary's absolute path
(`std::env::current_exe()`) into every hook command. The harness runs
that command with whatever `PATH` it started with, so an absolute path
is right, but the running binary's own path is often fragile:

- Installed with Nix, it is a store path
  (`/nix/store/<hash>-conspectus-<version>/bin/conspectus`; the
  `~/.nix-profile/bin/conspectus` the operator typed resolves to it).
  The next upgrade points the profile at a new store path, and garbage
  collection deletes the old one.
- Run with `cargo run`, or from `target/release`, it is a build
  directory, which `cargo clean` empties.

With Claude Code's fourteen events installed, every tool call and prompt
then runs a command that fails; Claude Code shows a hook error each
time, and live state stops without a word from Conspectus. Re-running
`hook init` did not help: without `--command` it keeps the command the
installed entries run. Research note 01 (section 4.2) recommended a
stable path.

The opencode plugin and the Pi extension embed no path: they run
`$CONSPECTUS_HOOK_BIN`, else `conspectus` from the `PATH` the harness
started with ([ADR 0049](0049-opencode-hook-plugin-distribution.md),
[ADR 0121](0121-pi-harness-support.md)), so an upgrade does not strand
them.

## Decision

1. **Fragile places.** A path is fragile when it is in the Nix store
   (`/nix/store`, or `$NIX_STORE_DIR`, which Nix itself honors), or in a
   cargo build directory: under `target/debug` or `target/release`, or
   under any directory holding a `CACHEDIR.TAG` (which cargo writes at
   the root of every target directory, a custom `CARGO_TARGET_DIR`
   included).

2. **The program `hook init` installs.** When the running binary is not
   fragile, its path, as before. When it is, the first of these that is
   an executable file, is not fragile itself, and is the same binary
   (the same file once links are resolved, or a file of the same
   content):
   1. `conspectus` in each absolute `PATH` directory, in order;
   2. `conspectus` in a Nix profile: `~/.nix-profile/bin`,
      `~/.local/state/nix/profiles/profile/bin`,
      `/etc/profiles/per-user/$USER/bin` (NixOS and nix-darwin), and
      `/run/current-system/sw/bin` (NixOS).

   The command records that candidate's own absolute path, the link,
   not what it resolves to, so the profile's next generation is what the
   hooks run. Failing every candidate, the running binary's path is
   installed and a warning on stderr says where it sits, that nothing
   stable is this build, and that `hook init` (or `--command`) is the fix
   once it is gone.

3. **`hook init` says what it chose.** Whenever Conspectus picked the
   command (a fresh install, or entries it moves), stdout gets
   `hooks run <program> (<why>)`, where `<why>` is `this binary`, `the
   conspectus on PATH, which is this build`, `the conspectus in your Nix
   profile, which is this build`, or `this binary, in the Nix store` (or
   `in a cargo build directory`). Moved entries add `, in place of <old
   program>, which no longer exists` (or where the old one sits).

4. **When installed entries move.** `--command` still moves every
   Conspectus entry. Without it, entries keep the command they run,
   except when a program they run is gone (a path to no file), or when
   every program they run is fragile and decision 2 found a stable
   candidate other than it. Then every Conspectus entry moves to the
   chosen command, as `--command` would move it, so re-running `hook
   init` repairs a stranded install and moves an older store-path
   install to the profile link. A bare name (`conspectus`) is never
   moved: the harness looks it up on its own `PATH`.

5. **`hook status` names a program it can't find,** on stderr, with its
   stdout lines unchanged: for Claude Code and Codex, each program the
   Conspectus entries run that is gone (`... which no longer exists, so
   each one fails; conspectus hook init <harness> installs a command that
   exists`), or a bare name this shell's `PATH` lacks; for the Pi
   extension and the opencode plugin, `$CONSPECTUS_HOOK_BIN` when it
   names a missing file, else `conspectus` when this shell's `PATH` lacks
   it, with the fix (set `CONSPECTUS_HOOK_BIN` or put one on `PATH`).
   The harness reads its own environment, so the notes say "this
   shell's".

6. **No new treatment for the plugin and the extension.** They resolve
   the binary when they run, so an upgrade leaves them working; decision
   5 covers the case where they cannot find it.

## Consequences

- A Nix user who installed `conspectus` into a profile gets hooks that
  run the profile's link and survive upgrades and garbage collection;
  one who installed before this change gets them by re-running `hook
  init`.
- A developer running `cargo run -- hook init` gets the build directory
  path, as before, plus a warning; with a link to the build on `PATH`,
  the link.
- Moving entries changes their command, so Codex asks for trust in
  `/hooks` again, which `hook init codex` already says after every run.
- Checking candidates reads up to a handful of files' metadata and, only
  for candidates of equal size whose links resolve elsewhere, their
  content.
- The new code lives in `src/cli/hook_program.rs` with unit tests on
  scratch directories; integration tests run a hard-linked copy of the
  built binary from a fake store with a profile link, and repair a
  missing program for Claude Code and Codex.

## Alternatives Considered

- **Install the bare name `conspectus`.** Survives every upgrade, but
  the harness runs hooks with the `PATH` it started with: a harness
  started from a desktop launcher, a service, or a shell that does not
  source the profile finds nothing, and every hook fails the same way.
  Kept as what `--command "conspectus hook write <harness>"` gives an
  operator who wants it.
- **A wrapper script in Conspectus's data directory** that execs the
  current binary. A new write path and a file to keep in sync, and the
  wrapper's own target still goes stale. Rejected.
- **Resolve the store path to the profile by asking Nix** (`nix profile
  list`). Starts a program for what a few file checks answer, and does
  not cover cargo builds. Rejected.
- **Only report, never move installed entries.** Leaves the operator to
  copy a `--command` line from the docs after every upgrade, which is
  what the troubleshooting guide asked before. Rejected.

## Open Questions Answered

- *Does `hook init` replace an operator's own command?* Only when its
  program is gone, or when it is a store or build path and a stable path
  runs this build (decision 4). `--command` remains the way to choose.
