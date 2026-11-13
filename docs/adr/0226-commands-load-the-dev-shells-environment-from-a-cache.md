# ADR 0226: Commands Load The Dev Shell's Environment From A Cache

## Status

Accepted. Adds `scripts/dev-env.sh`; adds no tool and no dependency. The
dev shell itself (`nix develop`, the flake, and the toolchain
[ADR 0110](0110-pinned-rust-toolchain.md) pins) is unchanged and stays
the source of the environment. The task runner of
[ADR 0010](0010-task-runner-selection.md) runs unchanged under the
cached environment.

## Context

The dev shell provides the pinned Rust toolchain, nextest, `just`, the
work tracker CLI, tmux, and strace. An agent session started outside it
has none of them, so agents wrap each command in
`nix develop --command ...`. Each wrap evaluates the flake and runs the
shell hook: 7.9 s per call with a warm store, measured 2026-11-12, and
7.2 to 8.7 s on 2026-11-13 (up to 12 s under load). In this project's
agent transcripts
from 2026-10-21 to 2026-11-12, 88% of 3,128 cargo commands were wrapped,
about six hours of shell start-up in three weeks. A command that is not
wrapped finds whatever `cargo` the user's own profile has, if any: 1.95.0
on the dev machine, not the pinned 1.99.0.

Each wrap also leaves two directories in `/tmp`: `nix develop` writes its
rc file into a `nix-develop-*` directory, and the rc file points `TMPDIR`
at a fresh `nix-shell.*` directory; neither is removed when the command
`exec`s. On 2026-11-13 the dev machine's RAM-backed `/tmp` held 8,631 of
the first and 8,633 of the second.

Constraints:

- **Every agent command starts a fresh shell.** No environment set by one
  command reaches the next; only what the agent process itself was started
  with is inherited.
- **Sessions start inside a sandbox** (`nono run --allow ... -- claude`).
  A fix that needs no change to how sessions start, and none outside the
  repository, works for every session and every agent harness at once.
- **The toolchain must stay the flake's**, and a change to `flake.lock`
  or `rust-toolchain.toml` must reach the commands that follow it.

`nix print-dev-env` prints, as a bash script, the environment
`nix develop` would give, and `--profile <path>` records it as a garbage
collector root. Sourcing that script takes about 10 ms.

## Decision

1. **A script loads the environment.** `scripts/dev-env.sh COMMAND...`
   runs a command in the dev shell's environment; `. scripts/dev-env.sh`
   loads it into the current shell, for a command line that runs several
   tools. It replaces `nix develop --command` for agents and scripts; a
   person may still enter `nix develop` for the banner and an interactive
   shell.
2. **The environment is cached per worktree, keyed by its inputs.** On
   first use the script runs `nix print-dev-env --profile` and keeps the
   output under `target/dev-env/` (git-ignored, removed by `cargo clean`),
   named by a hash of `flake.nix`, `flake.lock`, `rust-toolchain.toml`, the
   script itself, and the system. A change to any of them builds a new copy
   on the next call, in the seconds one `nix develop` takes; older copies
   and their profile links are removed. The profile link keeps the
   environment's store paths alive until the copy is replaced.
3. **Two parts of `nix develop` are left out.** The script removes, from
   the printed environment, the six lines that make a fresh temporary
   directory, point `TMP`, `TMPDIR`, `TEMP`, and `TEMPDIR` at it, and run
   the shell hook. Commands keep the caller's `TMPDIR` and leave no
   directory behind, and the banner is not printed on every call. The
   hook only prints the banner (the flake says so beside it). If Nix
   prints a script without those six lines, the script stops with a
   message instead of loading a partial environment.
4. **Nested calls cost nothing.** The script exports
   `CONSPECTUS_DEV_ENV` with the cache key and does not load the same
   environment twice, so a session or recipe already inside it runs
   commands directly.
5. **A session may start inside it.** Starting the agent through the
   script (`scripts/dev-env.sh claude`, or the same after
   `nono run --allow ... --`) should give every command the environment
   with no prefix, since commands inherit the agent's environment. That
   environment is fixed for the session; the prefix stays the way to pick
   up a change to the flake mid-session. The repository does not depend on
   sessions starting this way.

## Consequences

- A command reaches cargo in 0.04 to 0.05 s instead of 7 to 9 s (the
  work tracker CLI takes 0.36 s, its own start-up), and leaves nothing in
  `/tmp`, measured on 2026-11-13.
- The first command after a change to the flake or the toolchain file
  takes as long as one `nix develop` (about 10 s), once per worktree.
- `cargo clean` removes the cache with the rest of `target/`; the next
  call rebuilds it.
- Commands see the same variables as under `nix develop` except
  `TMPDIR` and its siblings, `NIX_BUILD_TOP`, and the hook's output. The
  variables build scripts read (`CC`, `AR`, `NIX_CFLAGS_COMPILE`, ...) are
  identical, so switching between the two rebuilds nothing: a test build
  through the script right after one under `nix develop` was fresh.
- Logic added to the shell hook later would not reach commands run
  through the script; the comment beside the hook says so.
- The script parses one detail of Nix's output, the six lines it removes,
  and fails closed if they change.
- The kept copy is shell code the script sources. It lives in the
  worktree's `target/`, with the same trust as the worktree's own scripts.

## Alternatives Considered

- **Start the agent inside the dev shell** (`nix develop --command
  claude`). One start per session, and commands need no prefix. It needs
  every session to be started that way, from outside the repository, and
  the environment is fixed for the session's life, so a flake change
  mid-session goes unnoticed. Decision 5 keeps this as an option on top of
  the script, at a tenth of the start-up cost.
- **direnv with nix-direnv and a checked-in `.envrc`.** nix-direnv caches
  `nix print-dev-env --profile` the same way. But direnv loads the
  environment from an interactive shell's prompt hook, and agent commands
  run in non-interactive shells that never run it, so each command would
  still need `direnv exec . COMMAND`. It also needs two programs installed
  on the machine and a `direnv allow` per worktree, outside the repository.
- **A cached profile used by hand** (`nix develop --profile` once, then
  `nix develop <profile> --command`). Skips flake evaluation but still
  starts Nix and the hook on each call, and leaves the same temporary
  directories.
- **A harness hook** that loads the environment when a session starts
  (Claude Code's session-start hook can write variables for later
  commands). It ties the fix to one agent harness and its configuration
  file, and it is configuration the operator, not the repository, should
  choose; decision 5 covers the same need for any harness.
- **Installing the toolchain outside Nix** (rustup on the host). A second
  source for the toolchain, which ADR 0110 retired.

## Open Questions Answered

- *Where does the cache live?* Under `target/` of each worktree: it is a
  build artifact of that worktree's flake, it needs no location outside
  the repository or the sandbox's allowed paths, and it goes away with the
  worktree.
- *How does a flake or toolchain change reach the next session?* By
  itself: the next call through the script sees the new hash and rebuilds
  the copy. A session started inside the environment (decision 5) picks it
  up when it is restarted.
