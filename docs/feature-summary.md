# Conspectus Feature Summary

A point-in-time snapshot of what Conspectus does today and what is planned
next. Update this file as phases land so it stays a useful "what is this
project, right now?" reference.

## What Conspectus is

Conspectus is an early-stage Rust CLI that surveys the AI-coding-agent
workflows alive on your local machine and emits the result as a single,
deterministic JSON graph. Each invocation walks read-only data sources — git,
the on-disk metadata that Atelier writes for its forks/worktrees, the local
session state of common agent harnesses, and `tmux list-sessions` — merges
everything into a provider-neutral model of repos, worktrees, branches,
workspaces, forks, agent sessions, and mux sessions, then runs a small
resolver to pick "preferred" relationships without throwing away the
alternatives.

## What it does today (through Phase 3)

- **One command, one graph.** `conspectus graph --format json` from a cwd
  (or with `--scan-root`) produces a deterministic JSON document with
  `nodes`, `candidate_links`, `resolved_relationships`, and `diagnostics`.
- **Git discovery.** Recognises plain repos, linked worktrees, detached
  HEAD, current branch, upstream, and remotes — without writing anything.
- **Workspace inference.** Detects both generic multi-repo roots (a
  directory containing several git repos) and Atelier workspaces (from
  `atelier.toml`), without inventing workspaces for standalone repos.
- **Atelier fork awareness.** Parses `.atelier/forks/index.toml` into one
  polymorphic `Fork` node per fork plus links for `forks_workspace`,
  `forks_repo`, `created_worktree` / `referenced_worktree`,
  `created_branch` / `associated_branch`, `rooted_at_path`, and
  `parent_fork`.
- **Agent session discovery.** Read-only adapters for `claude-code`,
  `opencode`, `codex`, and `aider` enumerate local sessions and emit
  `AgentSession` nodes carrying the real working directory. Codex walks
  `sessions/YYYY/MM/DD/`; Claude Code scans up to 200 JSONL lines for the
  first one with `cwd` (falling back to decoding the encoded project
  directory name); aider treats every scan root with `.aider.*` markers as
  a session. State roots come from `CONSPECTUS_<HARNESS>_STATE` overrides
  or `$HOME`-relative defaults. opencode currently only handles its legacy
  `storage/session/<id>/info.json` layout — modern installs use SQLite and
  are tracked by `P3-FU-002`.
- **tmux discovery.** Invokes `tmux list-sessions -F` through an injectable
  runner (tests use a `FakeTmux`) and turns each row into a `MuxSession`
  node with cwd plus activity/created epochs. `unavailable`, `no server`,
  and `failed` outcomes degrade gracefully.
- **Cross-provider link inference.** After every provider has contributed,
  a single pass infers `LinkedToMux` candidates when an agent session and
  a mux session share a working directory and `AssociatedWith` candidates
  when a session cwd sits inside an Atelier fork root.
- **Ambiguity-preserving resolver.** Implements ADR 0006: declared →
  strong → exact-cwd → naming convention → recency. When several mux
  sessions plausibly match one agent, every candidate stays in
  `candidate_links`, the preferred one shows up in
  `resolved_relationships`, and the rest are listed as
  `competing_link_ids` plus a `Conflict` diagnostic.
- **Lineage without placeholders.** Atelier harness lineage (parent/child
  session evidence with `native` / `approximate` / `unsupported` / `fresh`
  capability) is preserved as `ParentSession` / `ChildSession` candidate
  links with unresolved-endpoint metadata; Conspectus refuses to fabricate
  `AgentSession` nodes for evidence it has not actually discovered.
- **Determinism by construction.** Output is canonicalised (sorted
  nodes/links, stable ID shape) so the same machine state always renders
  the same JSON; tests rely on this with insta snapshots.
- **Tests instead of production-state coupling.** The harness fixture
  builders and `FakeTmux` mean the entire test suite runs without
  touching `~/.codex`, `~/.claude`, etc., and without needing a real tmux
  server.

## What's coming soon

- **opencode SQLite reader (`P3-FU-002`).** Modern opencode keeps sessions
  in `opencode.db` rather than the legacy `storage/session/<id>/info.json`
  layout. Adding a SQLite read dependency needs an ADR first.
- **Forge / PR awareness (Phase 4).** GitHub-style `ForgePr` nodes and
  `branch_has_forge_pr` links, plus the first non-JSON output (a tabular
  view of agent/mux/work state).
- **Declared links and overrides (Phase 5).** User-authored TOML files
  near a workspace/repo that pin a session to a mux, ignore a noisy
  candidate, or rename a fork — federated with the discovered evidence
  rather than replacing it.
- **Atelier command delegation (Phase 6).** Calling out to Atelier for
  fork operations so Conspectus stays the read-only "what's going on"
  tool while Atelier remains the writer.
- **Activity recency from real tmux/harness data.** The model already
  carries `activity_epoch` / `created_epoch` and the resolver already
  uses them as tie-breakers; once parsers populate them the recency
  ordering becomes meaningful in production.
- **Beyond JSON.** Agent/mux/union table projections that pick a default
  mux per session while exposing the ambiguity, plus eventual MCP/agent
  integrations.

## Real problems it solves today

Conspectus is most useful right now if you're doing any of the following:

1. **Auditing what AI sessions you've actually started on a machine.**
   Even with empty cwds, the tool lists every Codex/Claude/opencode/aider
   session that has on-disk state across your harnesses in one document
   — a question every existing harness only answers in its own silo.
2. **Inspecting Atelier fork state from outside Atelier.** If you use
   Atelier, you get a typed view of which forks exist, what
   worktrees/branches they materialised, and what session lineage
   Atelier intended (native vs approximate vs unsupported vs fresh)
   — without parsing TOML by hand.
3. **Locating tmux work.** `tmux list-sessions` is one command, but
   Conspectus normalises it into the same graph as your repos and forks
   and (once `P3-FU-001` lands) will join it to the agent sessions
   you've actually run there.
4. **Building higher-level tools.** Because the output is a stable,
   schema-versioned JSON document with explicit `candidate_links` /
   `resolved_relationships` / `diagnostics`, downstream tools
   (an agent-deck-style picker, a status bar, a "resume the last session
   in this repo" CLI) get a clean substrate instead of scraping each
   harness themselves.

## Known limitations

- No writes anywhere. Conspectus is read-only by design today; there are
  no commands to start work, persist preferred relationships, or edit
  discovered metadata.
- No forge/PR data and no table views — both land in Phase 4.
- No MCP server, no daemon, no caching layer.
- Harness adapters parse the synthetic fixture shapes; until
  `P3-FU-001` they will miss cwds and recency information that real
  Codex/Claude/opencode sessions actually carry.
