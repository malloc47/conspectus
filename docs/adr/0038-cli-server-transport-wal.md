# ADR 0038: CLI / Server Transport Under WAL

## Status

Accepted

## Context

`docs/backlog.md` Phase 7 (`P7-004`) called for an ADR settling the
continuous server architecture and the CLI ↔ server transport.
The original scope listed an open choice between Unix domain
sockets, file-based snapshot polling, or both — written under the
prior assumption of a JSON-canonical persistence model where both
the server and the one-shot CLI could be safe writers depending on
who held the lock.

ADR 0036 adopts SQLite as the embedded query engine. ADR 0037
makes SQLite the canonical persisted store under WAL mode. These
two decisions reshape the transport question:

- WAL mode natively supports **many concurrent readers plus one
  writer** across processes on the same host. The read path
  becomes trivial: any process opens `graph.sqlite` in read-only
  mode and queries it directly. No IPC needed for reads.
- Only one process at a time may write. When the server is
  running, it owns the writer connection. One-shot CLI mutations
  (rename, declared-link CRUD) need an IPC channel to the server
  during that window.

The transport question shrinks accordingly: only the **write
surface** needs IPC. Reads — which are the overwhelming majority
of CLI invocations (`session`, `table`, `node show`, `query`, the
TUI) — never need to talk to the server.

This ADR settles `P7-004`'s deliverable. It is the transport half
of the Phase 7 ADR pair; the persistence half is ADR 0037.

## Decision

The CLI / server transport is built on **WAL-mode concurrent reads
plus a Unix-domain-socket write proxy**. The
"absence-of-a-server-is-not-an-error" guarantee from
`docs/design.md` line 639 is preserved by construction because reads
never require the server.

### Read path

- Any conspectus process that needs to *read* the graph opens
  `graph.sqlite` directly with `SQLITE_OPEN_READONLY`.
- The standard pragma triplet from ADR 0037 applies on every open:
  `synchronous=NORMAL`, `busy_timeout=5000`,
  `wal_autocheckpoint=1000`. The `wal_autocheckpoint` is honored
  by writers only and is harmless on read-only connections.
- Read-only opens never block, never wait for the server, and never
  trigger the WAL checkpoint path. They observe the most recent
  committed snapshot.
- This applies uniformly to: `conspectus session`,
  `conspectus table`, `conspectus node show`,
  `conspectus query <sql>`, the interactive TUI, and any future
  read-only command.

### Write path

Writes flow through one of two channels, decided at runtime:

1. **Server running.** The server holds the SQLite writer
   connection for the lifetime of its process. One-shot CLI
   commands that need to mutate (rename, declared-link CRUD,
   `--refresh` cold rebuilds) connect to the server's Unix
   domain socket at `$XDG_RUNTIME_DIR/conspectus/server.sock` and
   send a length-prefixed JSON request. The server applies the
   mutation in its own transaction and returns a JSON response.
2. **Server absent.** The socket file does not exist (or the
   connection attempt fails immediately). The one-shot CLI
   acquires the SQLite writer lock itself, performs its
   transaction, and exits. WAL's `busy_timeout` covers the rare
   case of two concurrent one-shot CLI mutations.

The runtime decision is a single connection attempt at the socket
path. The fallback is automatic and transparent to the user.

### IPC protocol

- Transport: Unix domain socket at
  `$XDG_RUNTIME_DIR/conspectus/server.sock`. The socket is
  user-private (mode `0600`). When `XDG_RUNTIME_DIR` is unset,
  fall back to `$TMPDIR/conspectus-$UID/` with the same mode.
- Framing: length-prefixed JSON. Each frame is a 4-byte
  big-endian unsigned length followed by exactly that many bytes
  of UTF-8 JSON. Requests and responses use the same framing.
- Request shape (subject to refinement during implementation):
  ```json
  {
    "command": "rename" | "declare-link" | "ignore-link" | "refresh" | ...,
    "args": { ... },
    "id": "<uuid>"
  }
  ```
- Response shape:
  ```json
  {
    "id": "<uuid>",
    "result": "ok" | "error",
    "data": { ... } | null,
    "error": { "code": "...", "message": "..." } | null
  }
  ```
- Read commands are not supported over the socket. The server
  rejects them with a clear error pointing at the
  read-the-file-directly contract. This keeps the protocol's
  surface minimal and avoids two ways to do the same read.

### Server lifecycle and config

- The server is user-managed (systemd user unit, launchd agent,
  or a manual `conspectus serve &`). The CLI must not auto-spawn
  the daemon on regular invocations (per `docs/design.md` line
  638). Absence of a server is not an error.
- Configuration extends the existing TOML config with a
  `[server]` table:
  ```toml
  [server]
  socket_path = "..."   # default: $XDG_RUNTIME_DIR/conspectus/server.sock

  [server.intervals]
  harness = "5s"
  mux     = "5s"
  git     = "30s"
  forge   = "5m"
  ```
- Interval defaults match `docs/design.md` lines 622–629.
  Configurable per provider class; user overrides win.
- Provider failures are isolated. A broken `gh` binary,
  unreachable mux backend, or unreadable harness state
  directory must not halt unrelated providers. Each provider
  carries success/failure state, last-refresh timestamp, and
  back-off. These surface through the server's status response
  (handled in `P7-008`) and through the `diagnostics` table in
  the shared database.

### Failure modes

- **Socket connect fails.** Treat as "server absent" and take
  the writer lock directly.
- **Socket connects but server hangs.** A reasonable client-side
  timeout (e.g. 30 seconds for normal mutations, longer for
  explicit refresh) elapses, the client returns an error, and
  the user can either retry or `kill` the server. The client
  does **not** silently fall through to the direct-writer path,
  since the server may still hold the writer lock.
- **Server crashes mid-transaction.** SQLite's WAL recovery on
  the next open rolls back the partial transaction. The
  database file remains consistent.
- **Two concurrent one-shot CLIs without a server.** SQLite's
  `busy_timeout` plus the brevity of individual transactions
  makes contention rare; on rare collisions the loser retries
  per the timeout, and if it still fails, returns a clear
  error.

### Out of scope

- Event-driven refresh via filesystem watchers — `P7-009`
  remains a stretch goal independent of this ADR.
- A multi-host or NFS-mounted database. Conspectus stays
  single-user, single-machine (`docs/design.md` line 643,
  ADR 0036).
- A read API over the socket. Reads use the database file
  directly.

## Consequences

- The transport surface area is small: one Unix socket, one
  protocol, mutation commands only. Reads bypass the server
  entirely.
- The `conspectus serve` implementation (`P7-006`) becomes
  simpler than the original P7-004 scope assumed. The server's
  main loops are: (a) the per-provider refresh scheduler, (b)
  the socket-accept loop for mutation requests. No read-path
  fan-out, no snapshot-pollution rate-limiting.
- The CLI ↔ server snapshot read path (`P7-007`) becomes nearly
  trivial: opening `graph.sqlite` read-only is the same code
  whether the server runs or not. The story collapses to "is
  the file present? open it. is the server present? talk to
  it only for mutations."
- The "absence-of-a-server-is-not-an-error" guarantee is
  structural rather than incidental. A reader cannot tell
  whether a server is running by looking at the file or its
  WAL sidecar.
- `H-PROD-002` (cache layer for forge metadata, tmux, harness
  scans) folds naturally: each provider's cache row lives in
  the same SQLite file. Writes go through the server when
  running, the writer lock when not. No per-cache file-locking
  scheme.
- `docs/design.md` §"Continuous Operation Mode" (lines 606–647)
  needs rewriting after this ADR lands. The rewrite paragraph
  covers the WAL-driven read path and the Unix-socket write
  path explicitly.

## Alternatives Considered

- **Read API over the socket.** Rejected. Reads via WAL are
  zero-copy from the filesystem cache and need no
  marshalling; routing them through the socket would add
  latency, JSON serialization cost, and a code path that
  duplicates the in-process query engine. Worse, it would
  break the "absence is not an error" guarantee for reads.
- **File-based snapshot polling for reads (the prior
  P7-004-named alternative).** Rejected. With SQLite as the
  canonical store, the snapshot *is* the file; there is
  nothing to poll. The polling pattern was useful under a
  JSON-canonical model where the server might publish
  periodic snapshots; that model is gone per ADR 0037.
- **HTTP transport instead of Unix socket.** Rejected for v1.
  HTTP adds framing, routing, and an attack surface that a
  single-user single-machine model does not need. Unix
  socket with 0600 permissions matches the threat model.
  HTTP can be reconsidered if a remote API ever lands;
  that is a separate ADR.
- **No server at all; everything is one-shot.** Rejected
  because the continuous-mode story (`docs/design.md` §606–
  647) names cases where periodic refresh and event-driven
  pollers benefit from a long-lived process. The server
  optionality is preserved: users who do not want one can
  ignore it without losing functionality.

## Open Questions Answered

- **What carries first-write-wins semantics when two one-shot
  CLIs race without a server?** SQLite's `busy_timeout=5000`
  blocks the loser briefly; if the contention persists past
  the timeout, the loser returns an error and the user
  retries. In practice mutation commands complete in
  milliseconds and contention is rare.
- **How does a one-shot CLI know the server is alive vs. just
  socket-hung?** It does not need to know definitively. The
  `connect()` returning `ECONNREFUSED` or `ENOENT` (no
  socket file, or stale socket file with no listener) means
  "absent" and the CLI falls through to the writer-lock
  path. The `connect()` succeeding but the server failing to
  respond within the client timeout means "server running
  but unresponsive" and the CLI returns an error without
  falling through (since the server may still hold the
  writer lock).
- **What about stale socket files left by a crashed server?**
  Server startup unlinks the socket path before binding (or
  detects an existing listener and refuses to start with a
  clear message). Client `connect()` on a stale socket file
  with no listener fails fast with `ECONNREFUSED`; the
  fall-through to direct writer-lock works.
- **Does the server's writer connection ever release for the
  one-shot CLI to take?** No. The intended pattern is
  "server running → all writes go through the server." The
  fall-through to direct-writer is only for "server
  absent." The server does not voluntarily release its
  writer connection mid-life.
- **Could `conspectus serve` and a one-shot CLI ever
  deadlock?** Only if the server hangs without releasing
  the writer connection *and* a one-shot CLI bypasses the
  socket. By design the one-shot CLI does not bypass: if
  the socket exists, it uses it. If the socket exists but
  the server has hung, the one-shot CLI returns an error.
  No deadlock; user intervention required to recover.
- **What about test environments?** Tests that exercise the
  read path can run without a server. Tests that exercise
  the write path either use a temp `XDG_RUNTIME_DIR` to
  avoid colliding with a real server or run a test-only
  server fixture. Both patterns are implementation detail
  for the test suite, not policy.
