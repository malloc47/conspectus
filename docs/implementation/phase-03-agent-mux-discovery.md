# Phase 03: Agent And Mux Discovery

## Summary

Add discovery for supported agent harnesses and tmux sessions. Preserve all
session-to-mux candidates while selecting one preferred relationship for
projections.

## End-State Behavior

- Conspectus discovers `claude-code`, `opencode`, `codex`, and `aider`
  sessions where supported by local state.
- Conspectus discovers tmux sessions and their root paths when tmux is
  available.
- Agent sessions can remain orphaned; mux sessions can remain unlinked.
- Multiple plausible mux links are preserved, and ambiguity is visible in JSON.
- Fork session lineage evidence can remain unresolved without placeholder
  session nodes.

## Implementation Changes

- Add harness discovery adapters that emit Conspectus-native `AgentSession`
  nodes and source metadata.
- Add tmux discovery using `list-sessions` format output.
- Generate GraphLinks for session cwd/root matches, fork association, mux
  candidates, parent session evidence, child session evidence, and unresolved
  lineage endpoints.
- Implement session-to-mux resolver scoring from ADR 0006: local declared,
  global declared, strong process or provider evidence, exact cwd/root match,
  naming convention, then recency or activity correlation.
- Preserve unresolved session lineage metadata per ADR 0005.

## Tests

- Fixture-based harness discovery tests using synthetic state directories.
- Unit tests for native, approximate, unsupported, fresh, and not-yet-discovered
  lineage evidence.
- Unit tests for mux candidate scoring and ambiguity reporting.
- Integration tests with an injectable command runner or fake tmux executable.
- Snapshot JSON tests for orphan sessions, mux-only sessions, one-to-many mux
  sessions, and fork-linked sessions.

## Manual Checks

```sh
tmux new-session -d -s conspectus-smoke -c "$PWD"
cargo run -- graph --format json
tmux kill-session -t conspectus-smoke
```

Also run against real local harness state and confirm orphan sessions are still
rendered as useful graph nodes.

## Assumptions

- Tests should avoid depending on the user's real tmux server.
- Harness adapters should avoid making provider-private schemas part of the
  public graph model.
