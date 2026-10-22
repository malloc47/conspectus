# ADR 0117: A Capability Manifest With A Traced Backstop

## Status

Accepted. Makes ADR 0087 (the mutation envelope) enforceable. ADR 0143
supersedes ADR 0087; the manifest now enforces ADR 0143.

## Context

ADR 0087 bounds what Conspectus may write and lists what it must never do.
Its enforcement was a code-review habit: "`grep -rn std::fs::write` and check
that every hit traces back to a category." Nothing failed when a change added
a program, a socket, or a write path, so a new capability could land without
anyone deciding to allow it. The rules were also spread across ADRs and
module comments, with no single place that said what the binary is allowed
to do.

Conspectus starts processes from about fifteen call sites in eleven files.
It opens no network sockets itself: `gh` is its only network path. Its one
socket is the daemon's Unix socket, and it sends a signal in exactly one
place, the operator-initiated teardown.

"System calls" could mean the raw kernel interface, but the raw set is in the
hundreds and changes with glibc, the Rust version, the allocator, and the CI
image. An allowlist at that level would fail on toolchain updates instead of
on behavior changes.

## Decision

1. **One manifest.** `docs/capabilities.toml` lists everything Conspectus may
   do outside its own memory, each entry citing the ADRs that allow it:
   - the programs it starts, with the subcommands it may pass them;
   - the socket families and paths it uses;
   - the signals it sends;
   - the files and directories it writes;
   - a denylist of networking crates.

   Each allowance belongs to a context. `read` covers every command,
   including discovery and the daemon, and allows only rebuildable caches.
   `act` covers commands the operator runs to change something. `hook` covers
   `hook init` and `hook remove`. `dev` covers the debug-only scenario
   builder. The manifest is the reference for what the binary may do. It is
   written for review, and changing a capability means changing this file.
2. **One place starts processes.** `src/exec.rs` builds every subprocess,
   naming a `Program` whose key is a manifest entry.
3. **A source check, in every `cargo test`.** `tests/capabilities_static.rs`
   reads shipped code and fails on:
   - `Command::new` outside `src/exec.rs`;
   - `CommandExt`, `std::net`, or TCP and UDP types;
   - `fork`, `exec`, `socket`, `connect`, or `ptrace` through `libc`;
   - Unix sockets outside the daemon;
   - `libc::kill` outside the teardown.

   It also checks that `Program` and the manifest list the same programs,
   that every cited ADR exists, and that no denied crate is in `Cargo.lock`.
   This catches a new call at the commit that adds it.
4. **A trace of the built binary, as a backstop.** `tests/capabilities.rs`
   runs the binary under `strace -ff` and checks what the Conspectus process
   itself does against the manifest:
   - every `execve` and its arguments;
   - every socket, connect, and bind;
   - every `kill`;
   - every file opened for writing, created, renamed, or removed.

   It runs on a materialized showcase world, with a temporary home, temporary
   XDG directories, and stand-in `gh`, `tmux`, `zellij`, and `wt` scripts, so
   the run is deterministic and offline. The scenarios cover:
   - the dev scenario builder;
   - read-only commands, including the TUI with git previews;
   - the daemon and its clients;
   - operator actions;
   - hook installation.

   Each scenario is checked against the contexts it may use. This layer
   covers what source reading can't: dependency behavior, computed program
   names, and argument lists.
5. **Arguments are checked by subcommand.** Patterns match the arguments
   after a program's global options (`git -C <dir>`, `tmux -L <socket>`),
   and tmux's `;` chains are checked one subcommand at a time. `git checkout`
   fails even though `git` is allowed. Programs the operator chooses, such as
   `$PAGER`, are allowed as a class.
6. **CI runs the trace and fails without it.** CI installs `strace` and sets
   `CONSPECTUS_REQUIRE_STRACE`. A local run without `strace`, or where ptrace
   isn't permitted, skips with a message. The nix dev shell includes `strace`
   on Linux. On macOS only the source checks run.

## Consequences

- Adding a program, subcommand, socket, signal, or write path fails a test
  until the manifest allows it, and the manifest diff shows the change in
  review.
- The trace sees only the code paths its scenarios exercise. The source
  check and the single process-launch module cover the rest, and a new
  operator action should add a scenario.
- Children's behavior is theirs. `gh` reaching the network is allowed because
  `gh` is the only program the manifest marks as networked. The traced run
  uses a stand-in, so CI never contacts GitHub.
- Writing the manifest surfaced two cases the envelope's prose had not
  settled:
  - `hook init` writes `~/.claude/settings.json` and `~/.codex/config.toml`
    under ADR 0028, while ADR 0087's first prohibition named `~/.claude/`.
    ADR 0087 is amended to make hook installation the operator starts its
    category 5, which prohibition 1 excludes. The manifest records it as the
    `hook` context.
  - The dev scenario builder commits with `--no-gpg-sign`, but only in repos
    it creates, in debug builds.
- `strace` becomes a CI and dev-shell dependency, and the test takes a few
  seconds.

## Alternatives Considered

- **Allowlist raw syscalls.** It breaks on toolchain and runner updates
  rather than on behavior changes, and it says nothing about arguments or
  paths. Rejected.
- **`PATH` stand-ins alone.** They are portable, but they miss programs run
  by absolute path, and they don't see sockets, signals, or writes. They are
  used here only as fakes.
- **`LD_PRELOAD` interception.** It misses calls that bypass libc. Rejected.
- **seccomp or Landlock.** These enforce rather than record, crash on a
  violation instead of reporting it, and seccomp can't express path rules.
  They are a possible later runtime hardening, not a test.
- **eBPF tracing.** It needs root on CI. Rejected.
- **`cargo-deny` for the dependency check.** It is a new tool for one check
  that reading `Cargo.lock` already does. Rejected.
- **A network namespace for the trace.** Unprivileged user namespaces are
  restricted on current Ubuntu runners, and the trace already fails on any
  non-Unix socket. Rejected.

## Open Questions Answered

- **Strength versus cost:** program and subcommand, network, signals, and
  writes are asserted. Raw syscalls aren't.
- **Gating:** the trace blocks merges, since it takes seconds.
- **Real tools:** real `git`, and stand-ins for everything that would need a
  server or the network.
