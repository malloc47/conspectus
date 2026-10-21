# ADR 0137: Forge Fetches Stop On Daemon Shutdown

## Status

Accepted. Refines ADR 0136 (lazy forge fetches) and ADR 0080 (the
daemon's shutdown latch).

## Context

`conspectus serve` exits after every scheduler thread has seen the
shutdown latch (ADR 0080). A forge fetch (ADR 0136) runs its `gh pr list`
calls in batches and never looked at the latch, so a shutdown during a
fetch waited for it:

- Ctrl+C reaches the daemon's whole process group, so the in-flight
  `gh` calls die, but the fetch then started its next batches, which ran
  to completion: about a second per batch.
- SIGTERM reaches only the daemon, so even the in-flight calls ran on.
- The forge scheduler then merged the result under the writer lock and
  published `graph.bin`, with the killed calls recorded as failures. The
  next daemon seeds from that file.

## Decision

1. **The shutdown latch cancels forge fetches.** The daemon passes its
   latch to discovery as the forge cancel flag. Once it is set, a fetch
   starts no further batch of `gh` calls.
2. **In-flight `gh` calls are killed.** A system `gh` call, the
   signed-in-hosts lookup (`gh auth status`) included, polls the flag
   while it runs and kills its child (SIGKILL) when the flag is set. Its
   output is read on separate threads, so a large listing can't fill a
   pipe and stall the child; a cancelled call doesn't wait for those
   threads, since a process `gh` started may still hold the pipes.
3. **A cancelled fetch changes nothing.** It caches no listing, the forge
   scheduler skips its merge cycle, and a `refresh` whose fetch was cut
   short answers that the daemon is shutting down instead of publishing.
4. **SIGKILL to `gh` children is a manifest signal.** `[signals]` in
   `docs/capabilities.toml` lists it, citing this ADR. It goes only to a
   `gh` process Conspectus started, and only for a fetch being cancelled.

## Consequences

- `conspectus serve` exits within one poll interval of the latch (about
  50 ms for a fetch, 200 ms for the schedulers), under Ctrl+C or SIGTERM.
  On the operator machine, with a hosts lookup or four `gh pr list`
  calls in flight, it exited in 162-206 ms, against 2.4-3.3 s before.
- The `graph.bin` a shutdown leaves is the last snapshot published
  before it, never one with fetches the shutdown cut short.
- One-shot commands and the TUI pass no cancel flag and are unchanged.

## Alternatives Considered

- **Exit without joining the forge thread.** Safe for data, since
  snapshot writes are atomic and `gh pr list` only reads, but it leaves
  `gh` processes running after the daemon and can still publish a
  half-failed snapshot if the exit races the merge.
- **SIGTERM instead of SIGKILL.** Gentler, but needs `libc::kill`, which
  the manifest reserves for worktree teardown, for a read-only child
  whose output is about to be discarded.
