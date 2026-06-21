# ADR 0081: Filesystem Watcher Dependency For Event-Driven Refresh

## Status

Accepted

## Context

`P7-009` graduates the daemon from "everything polls on its
class interval" into "cheap local signals wake the scheduler
when the OS-level event fires." The targets are:

- Harness state directories (`~/.claude-code/`, `~/.codex/`,
  `~/.opencode/`, …). New sessions appear / disappear as the
  user starts and ends conversations.
- Git refs (`<repo>/.git/refs/heads/*`, `<repo>/.git/HEAD`).
  Branch updates change resolved-checkout / branch-PR
  relationships.
- `.conspectus.toml` and the user-level config file. Edits
  should propagate without a daemon restart (this dovetails
  with the deferred `--reload-config` work from `P7-008`).

Polling at the existing class intervals (harness 5s, git 30s,
forge 5m) is correct but introduces noticeable latency between
"the operator opened a new Claude Code session" and "Conspectus
notices it." Event-driven refresh closes that gap to milliseconds
without giving up the polling fallback for OSes / paths that
don't support kernel-level watchers.

Rust has no filesystem-watching primitive in `std`. The choice
is:

1. **`notify`** — the de-facto Rust cross-platform abstraction
   over inotify (Linux), kqueue (BSD/macOS), FSEvents (macOS),
   ReadDirectoryChangesW (Windows). Maintained under the
   `notify-rs` org, MIT/Apache, broad downstream usage.
2. **Per-platform direct FFI**: `inotify` crate on Linux,
   `kqueue` or `fsevent-sys` on macOS, etc. Smaller individual
   surface, but the daemon would need a per-OS shim and a
   dispatcher.
3. **Hand-rolled `libc::inotify_init1` + the macOS FSEvents C
   API**: no extra crates, but a substantial pile of `unsafe`
   FFI for code that already-existing crates handle correctly.

## Decision

Use `notify` (`notify = "8"`). The daemon's per-class scheduler
threads use it to wait on filesystem changes with a timeout
equal to the class interval. When the OS fires a change event,
the thread wakes immediately and runs its cycle; otherwise the
timeout fires and the thread runs a normal polling cycle.

### Failure mode: graceful fallback to polling

Watcher installation can fail for several reasons that are
*not* programmer errors and *must* not crash the daemon:

- ENOSPC / EMFILE — the kernel's inotify watch limit
  (`fs.inotify.max_user_watches`) is exhausted.
- EACCES — the path is unreadable to the daemon user.
- The path lives on a filesystem (NFS, some FUSE backends,
  some container overlay configs) that does not propagate
  filesystem events reliably.
- The path does not exist yet (a harness whose state dir is
  created on first use).

In any of those cases, the affected scheduler thread logs a
one-line warning and falls back to its plain interval-based
sleep. The polling cadence is the same as today, so the
operator-visible behavior is identical to layer B; only the
fast-path latency is lost.

### Abstraction boundary

A thin `Watcher` trait wraps the per-OS notify backend:

```rust
pub enum WatcherEvent { Changed, Timeout, ShuttingDown }
pub trait Watcher: Send {
    fn wait(&mut self, timeout: Duration) -> WatcherEvent;
}
```

The trait is the seam the daemon's scheduler talks to. A
notify-backed impl handles real watchers; a fake impl (a
queue of pre-canned events) drives the test surface so we can
exercise the watcher-available, watcher-fallback, and
watcher-saturation paths without relying on real OS calls in
CI.

## Consequences

- One new dep on `notify` plus its transitive
  `notify-types`, `inotify-sys` (Linux), `fsevent-sys`
  (macOS). All MIT/Apache, all maintained, all already
  widely used elsewhere in the Rust ecosystem.
- The daemon's first-write story is unchanged: the watcher
  abstraction wakes the scheduler thread, which calls the same
  `try_class_cycle` from P7-006 layer B. Per-class state, the
  writer Mutex, the post-cycle persist, and the rotate-on-
  cold-rebuild logic all carry through verbatim.
- The watcher is a *latency optimization*, not a correctness
  requirement. Provider failures, rate limiting, and cache
  consistency all rely on the polling cadence; watchers can
  only make refresh sooner, never later.
- macOS FSEvents has coarser granularity than inotify (whole-
  directory rather than per-file). For the harness target
  this is fine — any change in the state dir is a refresh
  trigger. For git-refs (a future target), a directory event
  on `refs/heads/` is still actionable.
- Watcher installation per-path means the daemon walks the
  configured harness state dirs at startup and installs one
  watcher each. Recursive watching is *off* by default to
  keep the watch-descriptor count predictable; harness state
  dirs are flat enough that one watcher per top-level dir
  catches every change we care about.

## Alternatives Considered

**Per-platform direct FFI (`inotify` + `fsevent-sys`).**
Rejected because the per-OS dispatch shim ends up
re-implementing a thin slice of `notify`'s public API anyway,
and `notify` is already audited / used in production by
thousands of downstream crates. The dependency footprint is
nearly identical (notify pulls the same per-platform crates).

**Hand-rolled FFI (no extra crates).** Rejected because the
unsafe surface (raw inotify event parsing, FSEvents callback
threads, lifetime management for the watcher handle) is large
and the per-OS semantic gotchas (move events, rename
sequences, kqueue watch limits, FSEvents coalescing) are
exactly what `notify` already handles correctly.

**Skip watchers, accept polling forever.** Tempting and what
P7-006 ships. Rejected because the operator-visible latency
between "I just opened a Claude Code session" and "Conspectus
sees it" is exactly the kind of friction the daemon is
supposed to remove. The 5-second harness polling cadence makes
the daemon feel "always one tick behind."

## Open Questions

- Should `notify`'s `RecommendedWatcher` (which picks the best
  backend for the platform) be the default, or should we pin
  inotify on Linux to keep behavior predictable? The
  `RecommendedWatcher` default is the right call — it's what
  every other notify consumer uses and the trade-offs are
  per-platform anyway. Revisit if a specific OS surface
  causes pain.
- Recursive watching for git repos (when we add the git-ref
  target): every discovered `.git/refs/heads/` would need a
  watcher. Whether to install one per repo or one recursive
  watch per workspace root is a decision the git-ref
  implementation will make. Out of scope for this ADR.
- Watch-descriptor budget: on Linux `fs.inotify.max_user_watches`
  defaults to a few thousand. If Conspectus's harness target
  alone consumes a noticeable fraction, surface it via
  `conspectus status` so operators can raise the sysctl. Not
  needed for v1; harness state dirs number in the single
  digits per operator.
