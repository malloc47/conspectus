# ADR 0013: Read opencode Sessions From SQLite

## Status

Accepted

## Context

Modern opencode releases store session metadata in
`~/.local/share/opencode/opencode.db`, with session rows in a SQLite
`session` table. Conspectus currently reads the older
`storage/session/<id>/info.json` layout, which means current opencode
installs can appear to have no sessions even when local state exists.

Conspectus needs to read this store while preserving the existing
discovery constraints:

- discovery remains read-only and best-effort
- tests must not depend on a user's real opencode state
- provider-private storage details stay inside the opencode adapter
- adding a project dependency requires an ADR per `CLAUDE.md`

SQLite is a file format, not an external service. Shelling out to the
`sqlite3` CLI would avoid a Rust dependency, but it would add a runtime
tool requirement and make tests depend on CLI availability. Parsing the
SQLite file directly is not realistic or appropriate for Conspectus.

## Decision

Conspectus will use `rusqlite` to read opencode's SQLite state store.

- The opencode adapter opens `opencode.db` with SQLite read-only flags.
- Missing databases, missing tables, malformed databases, and rows
  missing required fields degrade to no rows for that source instead
  of aborting discovery.
- The legacy `storage/session/<id>/info.json` reader stays in place for
  older opencode installs.
- If the same session id appears in both stores, the SQLite row wins
  because it is the modern source of truth.
- The dependency uses `rusqlite`'s `bundled` feature so the crate does
  not rely on a system SQLite development package being present in the
  dev shell or downstream build environment.

## Consequences

- Modern opencode sessions can populate `AgentSessionNode.cwd`, which
  lets existing session-to-mux and session-to-fork inference work
  without changing the graph model.
- The production binary gains SQLite code through `rusqlite` and
  `libsqlite3-sys`.
- Discovery remains read-only: Conspectus never creates, migrates, or
  writes the opencode database.
- Adapter tests can create a temporary SQLite database and exercise the
  real reader without touching `~/.local/share/opencode`.
- Future opencode schema changes are contained to the opencode adapter.
  If opencode renames required columns, discovery should degrade to
  sparse output until the adapter is updated.

## Alternatives Considered

- Continue reading only the legacy JSON layout. Rejected because modern
  opencode installs no longer expose sessions there.
- Shell out to `sqlite3`. Rejected because it adds a runtime CLI
  dependency and makes tests less self-contained.
- Parse SQLite files directly. Rejected because SQLite is a complex
  database format and `rusqlite` is the appropriate mature Rust
  interface.
- Add an optional feature flag for SQLite support. Rejected for now
  because opencode is already a supported harness; making modern
  opencode discovery conditional would make default behavior
  surprising.

## Open Questions Answered

- SQLite discovery is best-effort and must not fail the whole graph if
  the database is missing, locked, malformed, or using an unexpected
  schema.
- The legacy JSON parser remains supported.
- SQLite rows take precedence over legacy JSON rows for duplicate
  session ids.
