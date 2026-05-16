# ADR 0011: Forge Discovery Via The `gh` CLI

## Status

Accepted

## Context

Phase 04 introduces forge discovery so Conspectus can associate GitHub pull
requests with discovered repos and branches. To do that, Conspectus needs a
way to read PR state from a remote forge while remaining:

- read-only with respect to the user's repo and forge state
- safe to run without network access (returns a sparse graph instead of
  failing)
- testable without a real network round trip
- aligned with `docs/design.md`'s data-model-first posture, so provider
  reality stays in adapters and source metadata rather than the core graph

The Conspectus development shell already provides the GitHub CLI (`gh`),
which delegates authentication to the user's existing setup and exposes a
stable, paginated, JSON-shaped `gh pr list` interface. The repository also
already uses a similar injectable-runner seam for tmux discovery
(`TmuxRunner` with `SystemTmux` and `FakeTmux`).

Calling the GitHub REST API directly from Rust would require adding an
HTTP client and an OAuth/PAT flow that Conspectus does not own. CLAUDE.md
explicitly forbids introducing project dependencies without first
documenting the need and alternatives.

## Decision

Forge discovery delegates to the `gh` CLI through an injectable
`GhRunner` trait that mirrors the existing `TmuxRunner` seam.

- Production discovery uses a `SystemGh` runner that shells out to `gh`
  with `--json` output and parses the resulting JSON array.
- Tests use a `FakeGh` runner that returns pre-canned outcomes (or any
  custom `GhRunner` implementation), so no test requires a real `gh`
  install or network call.
- Outcomes are classified as `PullRequests(String)` (raw stdout to be
  parsed), `Unavailable` (binary missing or unauthenticated), or
  `Failed { code, message }` for unexpected non-zero exits. Discovery is
  best-effort: `Unavailable` and `Failed` outcomes degrade to no PR data
  and a diagnostic rather than failing the run.
- The forge adapter is restricted to GitHub for this phase. Non-GitHub
  forge providers remain out of scope and would each get their own
  adapter behind the same `ForgeAdapter` trait.

This decision is deliberately scoped to delegation. If Conspectus later
needs forge behavior that `gh` cannot provide (for example richer
review-state inference, cross-host federation, or a non-interactive
service account flow), revisit this ADR before adding an HTTP client.

## Consequences

- Conspectus inherits `gh` authentication, host configuration, and rate
  limiting without owning any of them. Users who can already run
  `gh pr list` in their working directory automatically get forge
  discovery.
- The codebase does not gain an HTTP client, a JSON-over-HTTP API
  surface, or a credentials store.
- Tests stay fast and offline because the runner seam is injectable.
- Conspectus depends on the operational stability of `gh`'s `--json`
  schema. If GitHub changes that schema, the adapter is the only place
  that needs to follow.
- Forge discovery cannot run on hosts that lack `gh` or that have an
  unauthenticated install. This is acceptable because the rest of the
  graph remains available and the missing PR rows surface as a
  diagnostic.
- Adding a second forge provider (GitLab, Gitea, etc.) will require its
  own adapter and runner. The trait shape is intentionally shared so the
  second provider does not force a redesign of the first.

## Alternatives Considered

- Direct GitHub REST or GraphQL client. Rejected for this phase because
  it would add an HTTP client dependency, an authentication flow, and
  per-host configuration that Conspectus does not currently own.
- Shell out to `git ls-remote` plus heuristics. Rejected because it
  cannot recover PR state, draft flag, or recency information that the
  resolver needs for ranking candidates.
- Read GitHub PR state from local `gh` cache files only. Rejected
  because `gh` does not commit to a stable on-disk cache format and the
  CLI is the documented integration surface.
- Defer forge discovery until a non-`gh` integration exists. Rejected
  because the dev shell already ships `gh`, the user-facing benefit
  (session ↔ PR association) is in scope for Phase 04, and the
  injectable runner keeps the choice reversible.

## Open Questions Answered

- Forge discovery should be best-effort: missing or unauthenticated `gh`
  must degrade to a sparse graph instead of failing the run.
- Forge adapters should sit behind a `ForgeAdapter` trait so multiple
  providers can coexist later without rewriting the discovery surface.
- The `gh` invocation should request explicit `--json` fields rather
  than parsing human-formatted output, so adapter changes are visible
  at the call site.
