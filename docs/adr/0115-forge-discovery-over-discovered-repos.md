# ADR 0115: Forge Discovery Over Every Discovered Repo

## Status

Accepted. Refines ADR 0011 (forge discovery via `gh`). Amended by ADR 0136:
the concurrency cap is `[forge] concurrency`, default 4.

## Context

`docs/design.md` asks discovery to "query GitHub for PRs associated with
discovered repo/branch pairs", and ADR 0011 says a failed `gh` call
degrades "to no PR data and a diagnostic". The GitHub adapter fell
short of both:

- It probed only the discovery roots, so repos found later through
  session and mux working directories (the `git::cwd` pass) were never
  queried. A root that merely contains repos yielded nothing.
- It ran `gh pr list` with its defaults: open PRs only, at most 30.
- It recorded the owner/repo of the first GitHub remote but let `gh`
  choose which repo to query, and never queried a fork's `upstream`,
  where PRs from the fork usually live.
- It linked a PR to a local branch by name alone, so a stranger's fork
  with a branch named `main` linked to the local `main`.
- Failures produced no diagnostic, so an empty PRs view looked the same
  as a repo without PRs.

## Decision

1. **Forge runs over the graph, after the working-directory pass.** It
   no longer probes roots. It takes every Repo node the earlier passes
   produced, reads its GitHub remotes from the node and its local
   branches from the graph's Branch nodes, and queries each remote. The
   provider keeps its `github` freshness key and cache, so a fresh
   cycle still skips it and reuses the prior slice.
2. **Every distinct GitHub remote, named explicitly.** Each query passes
   `--repo HOST/OWNER/REPO`, and each distinct remote of a repo is
   queried, so a fork's `origin` and `upstream` are both covered and
   each PR records the remote it came from.
3. **Open PRs and recent history.** Two queries per remote: open PRs
   with a limit of 100, and PRs of any state updated in the last 30
   days. Results merge by number.
4. **A PR links to a local branch only when its head is one of the
   repo's remotes.** The head branch must exist locally and the head
   owner/repo must match one of the repo's GitHub remotes. Otherwise
   the link stays unresolved, keeping its evidence.
5. **Bounded concurrency.** Remotes are queried a few at a time on
   scoped threads, so a machine with many repos doesn't serialize dozens
   of network calls or open them all at once.
6. **Failures are diagnostics.** A missing, unauthenticated, or offline
   `gh`, or a failed query, emits a `ProviderUnavailable` diagnostic
   naming the provider and reason. The TUI's forge chip shows it, and
   PRs from repos that did answer still load.

## Consequences

- PRs appear for every repo with work in flight, not just the launch
  repo, and merged or closed PRs show up for recently active branches.
- Forge discovery makes up to two network calls per GitHub remote per
  refresh, every five minutes by default. With many repos that is more
  `gh` traffic, bounded by the concurrency cap and the existing cache.
- PRs from forks into an `upstream` now enter the graph, filed under
  `upstream`.
- `Diagnostic` gains a variant, so the snapshot format version bumps.

## Alternatives Considered

- **Walk the roots for repos.** Duplicates the git and working-directory
  passes, and still misses repos outside the roots that sessions use.
- **Query per local branch (`gh pr list --head`).** Exact, but one call
  per branch.
- **`--state all` with one large limit.** One call, but in a busy repo
  the newest PRs crowd out older open ones.
- **Match heads by branch name only, as before.** Simpler, but
  mislinks fork PRs, which matters more once upstreams are queried.
