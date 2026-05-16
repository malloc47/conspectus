# Conspectus Feature Summary

A point-in-time snapshot of what Conspectus does today and what is planned
next. Update this file as phases land so it stays a useful "what is this
project, right now?" reference.

## What Conspectus Is

Conspectus is a Rust CLI and library that surveys AI-coding-agent work
alive on your local machine. It reads local state only: git repositories and
worktrees, Atelier workspace and fork metadata, supported agent harness state,
tmux sessions, and GitHub pull requests through the `gh` CLI.

The core output is a deterministic provider-neutral graph of repos,
worktrees, branches, workspaces, forks, agent sessions, mux sessions, and
forge PRs. Discovery records every plausible relationship as evidence first;
the resolver then selects preferred relationships without discarding weaker,
ambiguous, or unresolved candidates.

Phase 6 stabilized the public library contract so Atelier and future tools can
consume Conspectus deliberately instead of scraping CLI output or importing
incidental internals. The curated entry point is `conspectus::api`.

## Current CLI

```sh
conspectus graph --format json [--scan-root PATH]...
conspectus session [--projection {agent|mux|union}] [--scan-root PATH]...

conspectus declared list   [--store {all|project|user}] [--scan-root PATH]...
conspectus declared create --id ID --relation REL --source SRC --target TGT \
                           [--reason R] [--label L] \
                           [--store {project|user}] [--scan-root PATH]...
conspectus declared remove --id ID [--store {all|project|user}] [--scan-root PATH]...
conspectus declared confirm  --id CANDIDATE_ID [--store {project|user}] [--scan-root PATH]...
conspectus declared ignore   --id CANDIDATE_ID [--reason R] [--store {project|user}] [--scan-root PATH]...
conspectus declared override --id ID --overridden-by NEW_ID [--reason R] \
                             [--store {project|user|all}] [--scan-root PATH]...
```

Without `--scan-root`, every command discovers from the current working
directory. `session` defaults to the configured projection from
`.conspectus.toml` or user config, falling back to `agent`. The `declared`
mutation commands auto-select the nearest project store when `--store` is
omitted, falling back to user config for orphan relationships.

## What It Does Today

- **Deterministic graph JSON.** `conspectus graph --format json` emits a
  stable document with `nodes`, `candidate_links`,
  `resolved_relationships`, and `diagnostics`.
- **Session tables.** `conspectus session` renders human-readable table
  projections: `agent` (one row per agent session), `mux` (one row per mux
  session), and `union` (both node kinds with relationship status).
- **Read-only git discovery.** Recognizes plain repos, linked worktrees,
  detached HEADs, remotes, upstreams, current branch, and all local branches.
  Non-current local branches are graph nodes too, which lets PRs for sibling
  branches resolve to branch targets instead of staying unresolved.
- **Workspace inference.** Detects generic multi-repo roots and Atelier
  workspaces from `atelier.toml`, without fabricating workspaces for
  standalone repos.
- **Atelier fork awareness.** Parses `.atelier/forks/index.toml` into one
  polymorphic `Fork` node per fork plus links for workspace/repo scope,
  created or referenced worktrees, created or associated branches, fork roots,
  parent forks, and unresolved session lineage evidence.
- **Agent harness discovery.** Read-only adapters discover `codex`,
  `claude-code`, `opencode`, and `aider` sessions. Codex walks nested
  `sessions/YYYY/MM/DD/` state; Claude Code scans JSONL records for `cwd` and
  falls back to decoded project paths; opencode reads modern `opencode.db`
  SQLite state and the legacy `storage/session/<id>/info.json` layout; aider
  treats scan roots with `.aider*` markers as sessions.
- **tmux discovery.** Runs `tmux list-sessions -F` through an injectable
  runner and emits `MuxSession` nodes with cwd plus activity/created epochs
  when available. Missing tmux, no server, and failed commands degrade to
  sparse output.
- **GitHub PR discovery.** Runs `gh pr list --json ...` through an injectable
  runner, emits `ForgePr` nodes, and links PRs to matching local branch nodes.
  Missing `gh`, unauthenticated `gh`, non-GitHub remotes, and command
  failures degrade to no PR rows for that repo.
- **Cross-provider inference.** After provider discovery, Conspectus infers
  session-to-mux candidates from matching cwd/root evidence and associates
  sessions with Atelier forks when a session cwd sits under a fork root.
- **Ambiguity-preserving resolution.** Resolver precedence handles declared,
  strong discovered, convention, cached, session/mux, and branch/PR
  candidates. Losing candidates remain visible and conflicts are recorded in
  diagnostics.
- **Declared relationships.** A user-authored `[declared]` TOML section in
  `.conspectus.toml` (project) or `$XDG_CONFIG_HOME/conspectus/config.toml`
  (user) pins preferred relationships, suppresses noisy candidates, and
  records explicit overrides. ADR 0014 fixes the schema; ADR 0012 fixes
  precedence. Discovery loads both stores as `LocalDeclared` /
  `GlobalDeclared` candidates with `Active` / `Ignored` / `Overridden`
  state, preserving discovered evidence underneath. `conspectus declared`
  exposes `list`, `create`, `remove`, `confirm`, `ignore`, and `override`
  flows. Writes are atomic, sort links deterministically, preserve
  unrelated config sections, and prune the `[declared]` section (and the
  file itself) when the last link is removed.
- **Configuration and operations docs.** Runtime knobs are documented in
  `docs/operations.md`: provider toggles (`CONSPECTUS_DISABLE_TMUX`,
  `CONSPECTUS_DISABLE_FORGE`), harness state overrides, config precedence,
  and the current CLI surface.
- **Curated library API.** `conspectus::api` re-exports the common consumer
  workflow: discovery configuration, `discover_local_with`, resolution,
  graph JSON rendering, table rendering, config types, declared-link helpers,
  and graph model types. ADR 0015 defines the stable public surface and
  `docs/library-api.md` inventories pure modules, impure boundaries, and
  injection seams.
- **Distribution and repository policy.** ADR 0016 makes crates.io the
  intended steady-state distribution path, allows pinned git revisions for
  Atelier migration, and keeps path dependencies local-development only. ADR
  0017 keeps Conspectus in the current standalone repository and does not
  schedule a Phase 7 repository move.
- **Atelier migration support.** `docs/atelier-migration.md` maps
  `atelier session list`, `atelier mux status`, forge-related status
  surfaces, and graph-heavy `atelier status` behavior to Conspectus commands
  and library entry points. The Atelier-side work is tracked in Atelier's
  `docs/conspectus-delegation.md`.
- **Offline tests.** Harness fixtures, `FakeTmux`, and `FakeGh` keep tests
  independent of real home-directory state, live tmux servers, GitHub
  credentials, or network access. The Phase 6 Atelier delegation fixture
  snapshots graph JSON plus all three session table projections for a
  representative Atelier workspace with fork, harness, and mux evidence.

## Real Problems It Solves Today

1. **Auditing local AI-agent sessions.** Conspectus lists Codex, Claude Code,
   opencode, and aider session state in one model instead of one harness silo
   at a time.
2. **Joining sessions to terminal work.** When harness and tmux cwd evidence
   line up, the graph and session table show the preferred mux relationship
   while keeping ambiguous alternatives visible.
3. **Inspecting Atelier fork state outside Atelier.** Forks, worktrees,
   branches, roots, parent forks, and session-lineage evidence are normalized
   into graph nodes and links without depending on Atelier command modules.
4. **Seeing branch and PR context together.** GitHub PRs become graph nodes,
   and PR head refs are matched against all local branches, not just the
   currently checked-out branch.
5. **Building higher-level tools.** The JSON graph is stable and explicit
   about evidence, resolution, and diagnostics, so downstream tools can avoid
   scraping every harness, mux backend, and forge provider independently.
6. **Persisting user intent across runs.** Declared links let users pin a
   preferred session↔mux or branch↔PR mapping, ignore a noisy candidate, or
   override one declaration with another — all stored as readable TOML next
   to the relevant project (or in user config for orphan relationships)
   without ever discarding the underlying discovered evidence.
7. **Giving Atelier a migration path.** Atelier can keep owning workspace and
   fork mutation while Conspectus owns the read-only graph/session/mux/forge
   observability contract. The migration guide, public API facade, and
   comparison fixture give that delegation work concrete targets.

## Current Limits

- Conspectus does not start sessions, create forks, or edit provider
  metadata. The only files it writes are the user-authored
  `.conspectus.toml` / user-config TOML stores driven by the
  `conspectus declared` mutation commands; discovery itself stays
  read-only.
- Forge support is GitHub-only and delegates to `gh`; unauthenticated or
  missing `gh` means no PR rows.
- Atelier still needs to implement its command-level delegation or
  deprecation work. Conspectus now provides the replacement surfaces and
  tracker references, but it does not change Atelier command behavior by
  itself.
- There is no MCP server, daemon, or caching layer yet.
- The session table is intentionally compact. It exposes preferred
  relationships and ambiguity indicators, but richer filtering, sorting, and
  interactive workflows are still future work.

## What's Coming Next

- **Atelier command delegation.** Phase 6 completed the Conspectus-side
  contract and opened the Atelier-side tracker. The remaining work is in
  Atelier: decide whether overlapping commands delegate, deprecate, or stay
  Atelier-owned.
- **Operational integrations.** MCP/agent integrations, richer table views,
  and possible cache/index support remain later-phase work.
- **Backlog migration.** The only open backlog item is evaluating a move from
  `docs/backlog.md` to a structured tracker once task volume or coordination
  needs justify it.
