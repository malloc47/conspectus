# ADR 0111: The Launch Directory Is An Optional Input

## Status

Accepted. Refines ADR 0012 (project config lookup) and the scan-root
default that the CLI, TUI, and `serve` share.

## Context

The operator launched the TUI from a directory that was later removed.
Pin launches from that TUI then failed. The cause was general: about
thirty call sites read the process working directory with
`std::env::current_dir()?`, and on Linux that call fails with `ENOENT`
once the directory is unlinked. Every discovering command (`pin launch`,
`pin list`, `graph`, `table`, `node show`, `alias list`,
`declared list`, `worktree list`) exited with a bare
`Error: No such file or directory (os error 2)`. The TUI launches a pin
by running `conspectus pin launch` as a subprocess, which inherits the
TUI's dead directory and fails the same way.

`serve` had a slower version of the same failure. It captures its
launch directory as the default scan root, and `normalize_scan_root`
aborts the whole discovery run when any root is missing. A daemon whose
launch directory is deleted fails every scheduler tick and keeps
serving its last snapshot.

The CWD feeds four things today:

1. **The default scan root**, when no `--scan-root` flag and no
   configured roots are given.
2. **The project config anchor** (ADR 0012): settings come from the
   nearest `.conspectus.toml` above the CWD, layered over user config.
3. **The default target of authoring commands**: `mux new` and
   `mux launch` without `--cwd`, `worktree` subcommands without
   `--repo`, `hook` with `--scope project`, and the project-store
   fallback when declaring a link or pin with no project config nearby.
4. **The TUI's start-up orientation hint**, which highlights the group
   matching the launch directory.

The operator considered removing the CWD from roles 1 and 2, so the
graph and settings would be the same from any directory, and chose to
keep both. The launch directory stays a useful, cheap signal for "the
project I'm in". The requirement is that losing it must never break
anything.

## Decision

### 1. One helper owns the working directory

`src/cwd.rs` is the only place that calls `std::env::current_dir()`.
It exposes:

- `cwd::current() -> Option<PathBuf>`: the working directory when it
  still exists as a directory, `None` otherwise.
- `cwd::for_default_target(flag)`: for role 3. Returns the working
  directory, or an error that says it is gone and names the flag to
  pass (`--cwd`, `--repo`, `--scope user`, `--scan-root`).
- `ScanRoots`: explicit roots plus an optional launch-directory root,
  for role 1 in long-running processes (below).

`tests/cwd_hygiene.rs` fails on any other `current_dir()` call in
`src/`, the same way `tests/comment_hygiene.rs` enforces ADR 0100.
Commands that set a child's directory (`Command::current_dir`) are not
affected.

### 2. Scan roots: flags, then config, then the CWD when present

Survey commands resolve roots as before: `--scan-root` flags win, then
configured roots, then the launch directory. When the launch directory
is gone, discovery runs with no implicit root. Repos, checkouts,
workspaces, and PRs with agent activity still appear through observed
session and mux paths, and pins through the pin-store registry
(ADR 0090).

Explicit roots keep strict validation: a mistyped `--scan-root` still
fails at startup with `scan root does not exist`.

### 3. Long-running processes re-check the implicit root

The TUI and `serve` hold their roots as `ScanRoots`. Explicit roots are
used as given. The launch-directory root, captured at startup, is
included in a discovery run only while it is still a directory. A
directory deleted after launch is dropped on the next TUI refresh or
scheduler tick, and comes back if it is recreated at the same path.

### 4. Config falls back to user config

`ConfigLoader::load(anchor: Option<&Path>)` walks for a project file
only when there is an anchor. Callers pass `cwd::current()`, so a
missing launch directory means user config plus built-in defaults.
Project files are still read for project-scoped intent (pins, declared
links, aliases) through the paths discovery already locates.

### 5. Subprocesses don't inherit an implicit directory

Spawns that create sessions already pass their directory explicitly
(`tmux new-session -c`, `git -C`, `wt -C`). The TUI's resume action did
not: it ran the harness resume command in the TUI's working directory.
It now runs in the session's recorded cwd. The `conspectus pin launch`
subprocess keeps the inherited directory, which is safe now that the
child tolerates it, and still receives the pin's cwd as `--scan-root`.

## Consequences

- Every command, TUI action, and daemon tick works from a deleted
  directory. Survey output from a deleted directory matches a run from
  a directory outside any project.
- Authoring commands without an explicit target fail with a message
  that names the missing flag, not `os error 2`.
- The graph and settings still depend on the launch directory while it
  exists. That is deliberate (see Context).
- New code can't reintroduce the bug by calling `current_dir()?`; the
  hygiene test points it at the helper.

## Alternatives Considered

### A. Drop the launch directory as an implicit root

Build the graph only from flags, configured roots, global sources, and
observed paths, so output is identical from any directory. Rejected by
the operator: running `conspectus table` inside a repo should still
cover that repo even when it has no agent activity yet.

### B. Read settings from user config only

Stop walking for a project `.conspectus.toml` to get `[table]`, `[tui]`,
`[server]`, and `[worktree]`, and read `[worktree]` from the repo being
acted on. Rejected by the operator for now: it changes documented
ADR 0012 precedence, and tolerating a missing anchor fixes the bug.

### C. `chdir` to `$HOME` at startup when the CWD is gone

Repair the process directory once at launch. Rejected: it doesn't help
a directory deleted after launch, which is the reported case, and it
silently changes which project config applies.

### D. Fix only the call sites on the pin-launch path

Rejected: the same `current_dir()?` pattern sits in every survey
command and in `serve`, and the next feature would copy it again.

## Open Questions Answered

- A deleted launch directory is treated exactly like launching outside
  any project.
- Explicit scan roots stay strict; only the implicit launch-directory
  root is optional.
- Authoring commands keep "here" as their default target.
