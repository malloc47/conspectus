# Phase 02: Local Discovery

## Summary

Teach Conspectus to discover local project structure: git repositories,
checkouts, branches, generic workspace roots, and Atelier workspace or fork
metadata. Output remains graph JSON.

## End-State Behavior

- Running Conspectus from a git repo emits repo, checkout, and branch nodes.
- Running from an Atelier workspace emits workspace, repo, checkout, branch,
  and fork nodes.
- Atelier fork metadata maps to one polymorphic `Fork` node per provider fork.
- Research, selected, checkout-producing, and standalone repo fork-like contexts are
  represented without fabricating fake workspaces.

## Implementation Changes

- Add discovery traits and local discovery orchestration.
- Add read-only git probes by shelling out to `git` for common dir, worktree
  root, branch, remote, upstream, and worktree metadata.
- Add generic cwd and configured scan-root discovery.
- Add Atelier metadata readers for `atelier.toml` and
  `.atelier/forks/index.toml`.
- Map fork context effects using ADR 0003 and ADR 0004 relation kinds:
  `forks_workspace`, `forks_repo`, `created_checkout`,
  `referenced_checkout`, `created_branch`, `associated_branch`,
  `rooted_at_path`, and `parent_fork`.

## Tests

- Integration tests using temporary git repos and linked worktrees.
- Fixture tests for Atelier workspace metadata and fork index parsing.
- Resolver tests for created vs referenced checkouts.
- Resolver tests for research forks, selected forks, standalone repo forks, and
  associated branch links.
- Snapshot JSON tests for plain repo, generic workspace, and Atelier workspace
  fixtures.

## Manual Checks

```sh
cargo run -- graph --format json
```

Run the command from:

- a plain git repo
- a linked worktree
- an Atelier workspace with no forks
- an Atelier workspace with worktree-mode, selected, and research forks

Confirm the command does not write `.conspectus.toml` or modify provider
metadata.

## Assumptions

- Git shell commands are acceptable for v1 parity and lower early risk.
- Shared-library extraction from Atelier is deferred; small pure parsing logic
  may be ported into Conspectus-native modules.
