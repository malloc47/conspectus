# Phase 05: Declared Links

## Summary

Add durable user-declared relationships. Declared links guide resolution but do
not destroy discovered evidence. Read-only discovery must remain clean and
non-mutating.

## End-State Behavior

- Users can create, list, remove, confirm, ignore, and override links.
- Local declared links beat global declared links.
- Declared links beat discovered, convention, and cached links.
- Ignored links are excluded from preferred resolution but remain visible in
  detailed evidence.
- Read-only graph and session commands never create `.conspectus.toml`.

## Implementation Changes

- Add global config loading from `$XDG_CONFIG_HOME/conspectus/config.toml`.
- Add local declared-link loading and writing for `.conspectus.toml`.
- Add global declared-link storage for orphan or user-wide relationships.
- Add commands for link and unlink operations between agent sessions, mux
  sessions, PRs, workspaces, repos, worktrees, branches, and forks.
- Add commands or flags for confirm, ignore, and override operations.
- Implement nearest-store selection: project-rooted declarations local;
  orphan/global declarations global; caches global only.

## Tests

- Unit tests for nearest-store selection.
- TOML serialization round-trip tests for local and global declared state.
- CLI integration tests proving read-only commands do not write state files.
- CLI integration tests for link, unlink, ignore, confirm, and override flows.
- Resolver tests proving declared links win while discovered evidence remains
  available.

## Manual Checks

```sh
cargo run -- graph --format json
test ! -e .conspectus.toml
```

Then create a manual mux/session link, re-run graph output, and confirm:

- the declared link wins resolution
- the discovered candidate remains visible
- the declaration is written to the nearest appropriate store

## Assumptions

- Declared state uses TOML for the first implementation.
- Cache/index storage can be added later and should not become the only durable
  representation of user-authored intent.
