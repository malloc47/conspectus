# ADR 0219: Socket Clients Bound Each Request By What It Can Take

## Status

Accepted. Refines the client side of the daemon transport of
[ADR 0038](0038-cli-server-transport-wal.md) as amended by
[ADR 0082](0082-retire-sqlite-persistence-and-query-surface.md), and
extends to every request the bound the hook writer's `hook_ingest` call
already had ([ADR 0204](0204-the-live-state-store-folds-agent-events-per-session.md)
decision 6, [ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
decision 7). Adds no program, subcommand, socket, signal, or write path
(ADR 0117, ADR 0143). Amended 2026-11-13 (see the end): the bound
covers the connect too.

## Context

`conspectus serve` answers one request per connection over a Unix socket
(ADR 0038). The `status` and `snapshot` handlers answer from memory; a
`refresh` takes the daemon's writer lock and rebuilds, and a whole-graph
or forge refresh asks the forges first, which has no bound of its own.
Until now only `hook_ingest` set socket timeouts (2 s). `status`,
`refresh`, and `snapshot` waited for as long as the daemon took, so a
daemon that accepted a connection and never answered (stuck on its writer
lock, stopped, or wedged) hung `conspectus status`, `conspectus refresh`,
every one-shot command that reads the daemon's snapshot (`graph`, `table`,
`node show`), and the TUI's refresh worker.

One bound does not fit every request: a snapshot that takes five seconds
means the daemon is stuck, while a refresh that takes a minute may be
fetching from a slow forge.

## Decision

1. **Every client request has a bound**, set on each read and write of
   its connection, so a silent daemon ends the call within the bound, and
   a large answer that keeps arriving does not:

   | Request | Bound |
   | :- | :- |
   | `snapshot`, `status` | 5 s |
   | `refresh` of the `mux` or `harness` class | 15 s |
   | `refresh` of `git` or `forge`, or of the whole graph | 2 min |
   | `hook_ingest` | 2 s (unchanged) |

   The live classes rescan only local processes and harness directories,
   but run under the writer lock and so may wait behind another class's
   cycle; the rest may fetch from forges or scan every repository. The
   bound belongs to the request, not to the caller: the TUI's refresh of
   a class waits as long as `conspectus refresh --class` does.

2. **A timeout is its own outcome** (`ClientOutcome::TimedOut`), apart
   from a broken connection (`Transport`), so each caller can say what
   happened.

3. **What each caller does then:**
   - A one-shot command's and the TUI's snapshot fetch fall back to an
     in-process rebuild, silently, as when no daemon runs.
   - `status` reports that the daemon did not answer and exits 1.
   - `refresh` reports the timeout and exits 1. It does not refresh
     in-process, as it does on another socket error: the daemon may still
     finish the refresh, and a second rebuild would race it for
     `graph.bin` and ask the forges twice.
   - The TUI's refresh after a hand-off, `r`, or `Ctrl-R` logs a warning
     and shows an in-process rebuild, as on any other failed refresh.
   - The hook writer writes the spool itself, as before.

## Consequences

- No command or TUI worker waits indefinitely on a daemon that takes a
  request and goes quiet. A stuck daemon costs a one-shot command 5 s
  before its own rebuild, and a TUI refresh at most one class's bound.
- A `refresh` slower than its bound now fails while the daemon finishes
  it; the operator sees the result at the next snapshot read.
- A heavily loaded machine that takes longer than 5 s to answer a
  snapshot makes the caller rebuild in-process, which is correct but
  slower.
- `connect` itself is not bounded. A daemon whose accept loop is stopped
  answers nothing, but a connection still lands in the listener's
  backlog, so the call times out on its first read; only a backlog
  already full would block `connect`, which needs thousands of
  abandoned connections first. (Amended: the connect is bounded too;
  see the amendment.)

## Alternatives Considered

- **One bound for every request.** A bound short enough to make a stuck
  snapshot cheap cuts off legitimate forge refreshes; one long enough
  for those leaves the snapshot read hanging for minutes. Rejected.
- **A deadline for the whole call** instead of each read and write. It
  would also end a daemon that trickles an answer, but cuts off a large
  snapshot on a slow machine for no gain against the failure that
  matters, a daemon that sends nothing. Rejected; the per-read bound
  matches the existing `hook_ingest` call.
- **Fall back to an in-process refresh after a refresh timeout**, as
  after other socket errors. Duplicates a refresh the daemon may still
  complete and races its `graph.bin` write. Rejected.
- **A configurable bound** (`[server]` key or flag). No need has shown up
  for one; the bounds are generous for what each request does. Not now.

## Open Questions Answered

- *Does the TUI give up on a daemon refresh sooner than the CLI?* No: the
  bound is the request's. The TUI's live-class refreshes are the short
  ones.

## Amendment: the connect is bounded too (2026-11-13)

Each abandoned call leaves its connection in the listener's backlog
until the daemon accepts it. A daemon stopped (`SIGSTOP`, a suspended
terminal) while clients keep asking (a TUI's refresh worker every cycle,
a hook writer on every session start) fills even a backlog of thousands
in time, and the next `UnixStream::connect` then waits for the daemon to
accept, with no bound, since the read and write bounds start only once
connected. Everything above stands except
as follows.

1. **The request's bound covers the connect.** The client makes its
   socket, sets the request's bound as the socket's send and receive
   timeouts, and connects only then. Linux bounds a stream connect that
   waits for room in a full backlog by the socket's send timeout and
   then fails with `EAGAIN`, which the client reports as `TimedOut`,
   with the caller's fallback of decision 3. Elsewhere (macOS, the BSDs)
   a full backlog refuses the connection at once, which reads as no
   daemon, as it did before.
2. **Same socket, same manifest.** The connect goes to the same path as
   before and the manifest's `[sockets]` entry is unchanged. The socket
   is made with `libc` (already a dependency), because the standard
   library makes and connects a Unix stream in one call; the source
   scan of ADR 0117 now allows `libc::socket` and `libc::connect` in the
   daemon module, where the daemon's `UnixStream` already lives, and
   nowhere else.

Alternatives considered: connecting on a helper thread and giving up on
it after the bound, which needs no `unsafe` but leaves the thread blocked
until the daemon accepts or exits, one per call, so a TUI would gather
a thread and a descriptor every cycle while the daemon stays stopped; a
non-blocking connect retried until the bound, which still needs `libc`
and on Linux polls (a full backlog answers `EAGAIN` at once and gives
nothing to wait on); and a socket crate (`socket2`, `nix`), a dependency
for one call.
