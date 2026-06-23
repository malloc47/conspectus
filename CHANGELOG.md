# Changelog

All notable changes to Conspectus are tracked here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
Conspectus is pre-1.0, so the version cadence is "phase
boundaries land in `[Unreleased]` until a tag is cut."

## [Unreleased]

### Removed (breaking)

- **`conspectus query` subcommand** (P11-010). The user-facing
  SQL surface backed by SQLite is gone, along with the
  `--list-views`, `--similar-to`, `--load-extension`, and
  `--format {table,json,csv,tsv}` flags it carried. The
  replacement for ad-hoc graph inspection is `conspectus graph
  --format json | jq …`. The saved-view library
  (`v_sessions_with_repo`, `v_mux_attachments`,
  `v_pr_by_branch`, `v_fork_ancestry`,
  `v_workspace_member_repos`) and `docs/query-guide.md` retire
  with the subcommand.
- **Vector-search surface** (P11-010 / ADR 0082). The
  `embeddings` overlay table, `conspectus query --similar-to`,
  `--load-extension`, and `docs/vector-search.md` go with the
  query subcommand. The `sqlite-vec` dependency leaves the
  runtime distribution.
- **`graph.sqlite` persistence layer** (P11-011a). The
  canonical persisted artifact is now a single zero-copy
  binary at `$XDG_DATA_HOME/conspectus/graph.bin` (ADRs 0082,
  0083). The legacy `graph.sqlite{,-wal,-shm}` and `backups/`
  artifacts are auto-cleaned by `conspectus serve` on
  startup. Schema migrations, backup rotation, and
  `PRAGMA user_version`-driven cache invalidation are gone
  with the SQLite layer.

### Added

- **`graph.bin` zero-copy snapshot** (P11-004, ADR 0083). A
  single rkyv-archived `GraphSnapshot` written via atomic
  POSIX rename; daemonless consumers mmap it directly via
  `snapshot::open_mmap`.
- **`snapshot` socket command** (P11-006). `conspectus serve`
  serves its in-memory snapshot bytes verbatim to connected
  clients, base64-encoded under `data.bytes`. The TUI and
  one-shot CLI prefer this path when reachable.
- **Daemon warm-restart** (P11-009, delivered as part of
  P11-011a). On startup, the daemon mmaps `graph.bin` to seed
  its in-memory snapshot state so the first per-class refresh
  has a prior to evict from instead of paying a cold-rebuild
  cost. Missing-file / version-mismatch / validation failures
  fall through silently.
- **Legacy-cache cleanup** (P11-011a). The daemon unlinks
  leftover `graph.sqlite{,-wal,-shm}` and the `backups/`
  directory on startup so upgrading operator data dirs trim
  themselves over time.

### Changed

- **`conspectus refresh` / `status` flows now route through
  the socket** when `conspectus serve` is reachable, falling
  through to in-process cold rebuild otherwise. Both paths
  surface which one they took.
- **`--no-cache` / `--refresh` semantics** preserved across
  the SQLite retirement: `--refresh` forces a local
  cold-rebuild (bypassing the daemon-snapshot short-circuit);
  `--no-cache` suppresses the `graph.bin` write at the end of
  a cold rebuild.

### Unchanged

- User-authored TOML (`.conspectus.toml` for declared links
  and pins, user-level config for aliases) keeps its existing
  shape and location. Phase 11 changed *how the resolved
  graph is cached*, not *what the operator authors*.
- The hook sidecar's SQLite usage (`src/hook.rs`) and the
  OpenCode harness adapter's read-only access to OpenCode's
  own SQLite databases are independent of the retired
  `src/query/` persistence layer; both continue to work.

### Migration notes

See [`docs/operations.md`](docs/operations.md#migration-from-earlier-0x)
for the full operator-facing migration write-up.
