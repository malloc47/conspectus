# ADR 0135: Pin Commands Skip Forge Discovery

## Status

Accepted. Follows up ADR 0133 (launches attach before they refresh).
Amended by ADR 0141 and ADR 0142: `conspectus rebuild` replaces `pin
launch` and `pin attach`; `rebuild` and the `pin` commands both still skip
forge discovery.

## Context

ADR 0133 removed the fresh-launch watch and the pre-attach refresh from
a TUI pin launch, about 1 s together. The operator saw no speed-up. The
TUI launches through `conspectus pin launch <id> --no-attach`, and that
subprocess starts with a full local discovery. A one-shot command has no
forge listings cached (ADR 0116), so it ran `gh pr list` twice for every
GitHub remote of every discovered repo before it touched tmux. With the
enterprise hosts ADR 0128 added, that was about 13 s on the operator
machine, against 0.6 s with forge disabled.

Pin resolution reads harness, mux, git, and process state. No pin
command reads PRs.

## Decision

The pin commands (`launch`, `attach`, `list`, `show`, and the edit
commands that resolve pins first) discover with every provider except
the forge adapters. Discovery for every other command is unchanged:
`declared confirm` and `ignore` can target a PR candidate, and `table
prs` and `graph` show PRs.

## Consequences

- A pin launch from the TUI or the CLI reaches tmux about 13 s sooner on
  the operator machine, and no longer depends on the network or `gh`.
- `pin list` and `pin show` don't show PR-derived diagnostics, which they
  never printed.
- The remaining 0.6 s is the rest of discovery, which the binding
  decision needs fresh (an out-of-date binding could type the launch
  argv into a live harness, ADR 0028).

## Alternatives Considered

- **Use the daemon's snapshot when one is running.** Faster still, but
  the snapshot can be a tick old, and pin launch decides whether to
  send keys from the binding.
- **Cache forge listings across one-shot commands.** Fixes every command
  at once, but persists data ADR 0116 kept in memory, for a command
  that doesn't need it.
