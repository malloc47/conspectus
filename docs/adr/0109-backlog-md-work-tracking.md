# ADR 0109: Backlog.md Work Tracking

## Status

Proposed. Accepting it supersedes ADR 0009. Drafted on the
`backlog-md-migration` evaluation branch; the operator accepts or rejects it
after evaluating the migrated backlog, before the branch merges.

## Context

ADR 0009 made `docs/backlog.md` the interim tracker and named Backlog.md
the preferred next tool once the project needed structured queries,
dependency operations, or agent integration. It also set the first
migration target: the existing `docs/backlog.md` content, not a parallel
issue database.

By 2026-10-02 the file held 622 stories in 15,387 lines, and 49% of all
commits touched it. A single file that every story edit rewrites is a
merge-conflict hotspot when several agents work in parallel worktrees,
which is ADR 0009's trigger. The operator also wants to evaluate
Backlog.md on the real backlog before the 0.1.0 talk.

Backlog.md (v1.53.0) constrains how the content can move:

- Task IDs are `<prefix>-<n>` with one letters-only prefix; subtasks are
  `<parent>.<nn>`. The project's per-workstream IDs (`P8-014`,
  `H-PIN-TUI-011`, `T8-043a`) do not fit: files named after them are not
  loaded, and IDs whose prefix contains a digit are silently rewritten to
  `TASK-P8-014`.
- Only files named `<prefix>-*.md` load, and the frontmatter serializer
  writes known keys only, so an extra key such as `legacy_id` is dropped
  on the first CLI edit.
- Filenames embed the full title, so a very long title cannot be saved.
- There is no importer.

## Decision

Adopt Backlog.md as the work tracker and convert every story, so the tool
is evaluated on the real backlog rather than a sample.

1. **IDs.** Every story is renumbered to `CSP-NNN`, zero-padded to three
   digits, in the order stories first landed on `main`'s first-parent
   history. A story keeps its first date across edits and renames (the
   planning stories were `P0-001`..`003` before becoming
   `PLAN-001`..`003`). Lettered sub-stories become subtasks of their
   parent (`T8-043a` becomes `CSP-NNN.01`). `P8-012` and `P11-011`, which
   survive only as lettered sub-stories, are restored as parents with
   their last historical titles. The two stories each that shared
   `H-MUXPROC-016` and `H-MUXPROC-017` become four tasks.
2. **Legacy IDs.** Each task keeps its legacy ID on a `Legacy ID:` line
   outside the structured sections. No CLI flag edits that line, so it
   survives edits; `task view` does not show it, but `git grep` finds it.
   `docs/backlog-legacy-ids.md` maps every legacy ID, with its first
   landing time and commit. Every reference in the repository (code
   comments, ADRs, docs, ASCII dependency diagrams, which stay aligned) is
   rewritten to the new ID. Commit messages are history and keep the
   legacy IDs; the map bridges them.
3. **Structure.** Each `##` section of the old file becomes a milestone
   whose description keeps the section's prose, with each story collapsed
   to a `- **CSP-NNN** Title` line in its original place. Each story
   becomes a task file:
   - title: the story's first sentence (clipped with `…` past 120
     characters, with the full sentence kept in the description);
   - description: the story's fields (`Scope`, `Tests`, `Blockers`, ...)
     verbatim;
   - final summary: the `Outcome` notes;
   - dependencies: the IDs in `Blockers:`, except those named after
     phrases such as "pairs naturally with" or "in parallel with", or
     made conditional with "if";
   - labels: the legacy workstream family (`p8`, `h-pin-tui`), plus
     `wont-do` or `obsolete` for the stories marked `[~]` or `[-]`;
   - priority: the 0.1.0 release tier (`P0` high, `P1` medium, `P2` low);
   - ordinal: the story's position in the old file.
   Every task goes to `backlog/tasks/`, the done ones in the `Done`
   column, so search, milestone progress, and subtask lists cover the
   whole history. `docs/backlog.md` becomes a pointer that keeps its old
   preamble.
4. **Agent integration.** Backlog.md's CLI instruction mode: `backlog
   init` adds its marked block to `AGENTS.md`, and project conventions
   for the tool sit outside that block. No MCP server is configured.
   Cross-branch scanning and remote fetches are off, and auto-commit stays
   off so task edits ride in the commits that make them. The dev shell
   pins Backlog.md as a flake input.
5. **Migration tooling.** `scripts/backlog-md-migration/migrate.py`
   generates the renumbering and the split deterministically and checks
   that the split accounts for every line of the renumbered file. The
   true-up before merge re-runs it on a fresh `main` instead of rebasing
   its output.

## Consequences

- Story edits touch one small file, so parallel branches rarely conflict
  on the backlog.
- `backlog task list --ready`, the dependency graph, `backlog doctor`,
  the board, and the browser UI work on the real backlog.
- IDs no longer name their workstream. Labels and milestones carry it,
  and `CSP-NNN` order reads as filing order.
- Agents write tasks through the `backlog` CLI rather than editing one
  Markdown file, and each session pays for reading Backlog.md's workflow
  overview.
- The board's `Done` column holds the whole history, about 500 tasks.
  Agents search or filter by status rather than listing everything.
- A third-party tool built on Bun enters the dev shell. It is pinned
  through `flake.lock` and builds from source.
- Commit history before the migration cites legacy IDs; readers use the
  map or the `Legacy ID:` lines to follow them.

## Alternatives Considered

- **Keep `docs/backlog.md`.** Rejected: the file has outgrown review and
  merges, and ADR 0009 named this migration as the next step.
- **Split the file into active and archive files without a tool.**
  Relieves the size and most conflicts, but gives no ready queue,
  dependency checks, or agent integration.
- **Migrate open stories only and freeze the rest.** Smaller, but the
  dependency check fails closed on IDs that no task claims, and the
  operator wants the whole corpus evaluated.
- **Done stories in `backlog/completed/`.** Keeps the board to open
  work, but Backlog.md's search, milestone progress, and subtask lists
  skip that folder, so 503 stories would drop out of search and finished
  phases would show `0/0`. Backlog.md's own workflow also leaves done
  tasks on the board until someone runs its cleanup.
- **Keep legacy IDs as task IDs.** Backlog.md loads them only through an
  undocumented path and rewrites any ID whose prefix has a digit.
- **Number by file position.** Adjacent numbers would mirror the old
  layout, but the numbering could not be recomputed at the true-up
  without a frozen map, and IDs created later would follow a different
  rule. `ordinal` keeps the file order anyway.
- **Legacy ID in the title, or in `references:`.** The title is visible
  on the board but lost on the first retitle; `references:` is replaced
  wholesale when an agent adds a reference.
- **MCP integration.** Avoids shell quoting, but needs a registration per
  harness (Claude Code, Codex, opencode) and adds about 20 KB of tool
  schema wherever it is registered. Can be added later for one harness.

## Open Questions Answered

- Backlog.md replaces `docs/backlog.md` as the tracker; GitHub Issues stay
  optional for public-facing work.
- Legacy IDs live in the map, on each task's `Legacy ID:` line, and in
  commit history.
