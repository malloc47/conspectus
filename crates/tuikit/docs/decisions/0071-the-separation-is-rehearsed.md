# 0071: The move to a repository of its own is rehearsed on every change

| | |
| --- | --- |
| Status | Accepted |
| Page | [separation.md](../separation.md) |

## Decision

- While the crate shares a repository with its host, the host's CI exports
  it on every change into a scratch repository of its own, with its
  history, and runs the crate's checks there with nothing of the host
  reachable: formatting, clippy with warnings denied (every feature, and the
  features off), the tests and doctests (every feature, and the features
  off), and the docs with warnings denied.
- The export makes the manifest stand alone: inherited package fields,
  dependency entries, and lint tables become concrete values, and an empty
  `[workspace]` table makes the package its own workspace root. Any
  inherited spelling the export does not rewrite stops it. The root files
  (toolchain, license, ignore list, CI workflow, `justfile`, dev shell) are
  written from the one list of checks the rehearsal runs, and the lockfile
  is the host's, pruned to the crate's packages.
- The rehearsal fails when a path that climbs out of the crate would still
  find the host's files, when tool configuration above the export would
  apply to it, or when the export resolves other packages or features than
  the host does for the crate. A self-test injects a compile-time and a
  run-time leak that both resolve in the host's layout and requires each
  to fail the rehearsal.
- The vocabulary guard skips lock files by name (`Cargo.lock`,
  `flake.lock`): they list third-party package names the crate does not
  choose, and a repository of its own commits them.

## Rationale

The independence guards see names, dependencies, and calls inside the
crate's directory. They cannot see what only resolves because of where the
directory sits: an inherited manifest field, a relative path out of the
crate, a test that assumes its location on disk, a root file the workspace
supplies. A move that has never been done fails on exactly those, and a
rehearsal done once stops being true at the next change, so it runs on
every change. The first run found a lock file the guard read and a test
that assumed its absolute path.

## Consequences

- Code, tests, and pages refer only to paths inside the crate; a file they
  need from elsewhere is passed in by the caller or moved into the crate.
- The manifest inherits from the workspace only in the spellings the
  export rewrites; a new cargo feature or a new spelling changes the
  export in the same change.
- The real move starts from the export the rehearsal makes; the steps are
  in [separation.md](../separation.md).

## Alternatives considered

- **A rehearsal done once, by hand.** It proves the move for one commit.
- **Copying the directory without history.** Simpler, but the log and the
  blame are part of what leaves.
- **An allowlist entry per lock file and word.** A lock file's words change
  with every dependency update; skipping the machine-written files by name
  keeps the list of exceptions about the crate's own text.

## Origin

- host ADR 0199, "The separation is rehearsed in CI": the export, the
  standalone manifest, the root files, the isolation checks, the checks and
  the self-test, and the lock-file rule (decisions 1 to 6 and 10).
