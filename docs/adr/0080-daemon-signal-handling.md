# ADR 0080: Daemon Signal Handling Via `signal-hook`

## Status

Accepted

## Context

`CSP-142` introduced `conspectus serve`, the long-running daemon
that keeps `graph.sqlite` warm between one-shot CLI invocations.
Layers A and B (the warm-start tick loop and the per-class
scheduler) shipped without any signal handling: SIGINT and
SIGTERM kill the process abruptly. Because SQLite WAL recovery
makes the on-disk graph consistent on every open, the worst case
today is losing the in-flight cycle's discovery work — which the
next start redoes anyway.

Layer C changes the picture. The mutation socket (ADR 0038) adds
real per-process state that wants draining on shutdown:

- An open Unix-domain socket file at
  `$XDG_RUNTIME_DIR/conspectus/server.sock` that must be
  `unlink`ed so the next `conspectus serve` invocation can bind
  again without first asking the operator to clean up a stale
  socket file.
- In-flight mutation requests that should either complete or
  return a clear error rather than dying mid-transaction (the
  client is blocked waiting on a response frame).
- A writer connection holding open SQLite handles that benefit
  from explicit close + checkpoint rather than crash recovery.
- An optional "final persist before exit" so the in-memory
  snapshot the scheduler has been accumulating reaches disk
  cleanly.

None of those are reachable without observing SIGINT/SIGTERM
inside the daemon process. Rust's standard library does not
provide a signal API; the daemon needs either a crate dependency
or hand-rolled `libc::signal` calls.

This ADR settles the choice ahead of the layer-C implementation
so the dep addition has a documented rationale per the project's
"no new dependencies without an ADR" guardrail.

## Decision

Use the `signal-hook` crate (`signal-hook = "0.3"`) for daemon
signal handling. The daemon registers SIGINT and SIGTERM against
a shared `Arc<AtomicBool>` shutdown flag at startup. Every
scheduler thread polls the flag between cycles; the socket
listener thread polls between `accept` calls. When the flag
flips, each thread completes its in-flight work and exits, the
socket file is unlinked, the final snapshot is persisted, and
the daemon returns cleanly.

The flag-based pattern matches `signal-hook::flag::register`'s
documented happy path:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;

let shutdown = Arc::new(AtomicBool::new(false));
flag::register(SIGINT, Arc::clone(&shutdown))?;
flag::register(SIGTERM, Arc::clone(&shutdown))?;
// ... threads poll shutdown.load(Ordering::Relaxed)
```

## Consequences

- One new dep on `signal-hook` (and its transitive
  `signal-hook-registry`). Both are MIT/Apache, maintained,
  widely used, and small.
- `conspectus serve` becomes Unix-only at the signal-handling
  layer. The project is already single-user / single-machine
  per `docs/design.md` line 643 and ADR 0036, and the existing
  socket / `$XDG_RUNTIME_DIR` story is already Unix-shaped per
  ADR 0038, so this is not a fresh portability tax.
- The "shutdown is observable" property unblocks layer C's
  socket cleanup, in-flight request drain, and final-persist
  semantics. Without it the mutation socket would leak its
  socket file across daemon restarts and operators would have
  to manually `rm` it.
- Future signals (SIGHUP for config reload, SIGUSR1 for "dump
  scheduler status to stderr") slot into the same registration
  pattern without further ADR overhead — the dep selection is
  the load-bearing decision.

## Alternatives Considered

**`ctrlc` crate.** Smaller surface (Ctrl-C plus, with a feature
flag, SIGTERM). Rejected because the daemon already needs
multi-signal handling (SIGTERM today, SIGHUP / SIGUSR1 / SIGUSR2
likely tomorrow) and `signal-hook` covers that uniformly. A
single dep that handles the present need and the obvious
follow-ups is preferable to swapping crates later.

**Raw `libc::signal` calls.** No new dep. Rejected because
async-signal-safe signal handlers must avoid almost every Rust
construct (no allocation, no panic, no most-of-the-stdlib);
getting that right by hand is error-prone and the project's
test suite cannot easily cover signal-handler safety.
`signal-hook` does the unsafe work once, audited.

**No signal handling — rely on SIGTERM-then-WAL-recover.** The
status quo through layers A and B. Rejected for layer C because
the mutation socket adds non-database state (the socket file,
open client connections, a writer thread blocked on a channel
read) that crash recovery cannot reach. The next daemon start
would find a leaked socket file and refuse to bind.

**`tokio`'s signal API.** Pulls in a full async runtime that
the daemon does not otherwise need. The scheduler is happy with
plain threads + sleep; introducing async just for signals
inverts the dependency cost.

## Open Questions

- Should the daemon also handle SIGHUP for config reload?
  Deferred until the operator pain actually shows up; for now
  reload is "kill + restart" and the systemd / launchd unit
  handles that.
- Should SIGUSR1 dump scheduler status to stderr (or to a path
  named by `--status-fd`)? CSP-144 will own this question; this
  ADR's signal-hook adoption pre-positions for it.
