# Backlog.md Migration: Review Notes

Temporary. Read before the true-up; delete this file, `rewrite-report.md`,
and `split-report.md` before merging.

Branch `backlog-md-migration` (worktree `../conspectus-backlog-md`), cut
from `main` at `82792fb`. Nothing outside the worktree was changed: no
user-level config, no MCP registration, no profile installs.

## Commits

| # | Commit | Kind |
| --- | --- | --- |
| 1 | `docs(adr): propose Backlog.md work tracking (ADR 0109)` | hand |
| 2 | `chore(scripts): add the Backlog.md migration converter` | hand |
| 3 | `test(hygiene): recognize CSP backlog IDs` | hand |
| 4 | `docs: drop backlog IDs from comments the hygiene check missed` | hand |
| 5 | `docs(backlog): renumber backlog IDs to CSP-NNN` | generated (`rewrite`) |
| 6 | `docs(backlog): split docs/backlog.md into Backlog.md tasks` | generated (`split`) |
| 7 | `chore(nix): add Backlog.md to the dev shell` | hand |
| 8 | `chore(backlog): initialize Backlog.md` | `backlog init` output |
| 9 | `docs: point agents and docs at the Backlog.md backlog` | hand |
| 10 | this file | temporary |

Commits 5 and 6 have been regenerated three times: twice as commit 4
was inserted and extended beneath them, with backlog output matching the
first run byte for byte (the property the true-up relies on), and once
to keep done tasks on the board, which moved 503 files without changing
their content.

## What was checked

- `migrate.py verify`: every story, field, outcome note, section, and
  preamble line of the renumbered `docs/backlog.md` is in the output.
- `backlog doctor` (v1.53.0): no duplicate IDs, self-references, or
  cycles. 121 open tasks, 86 of them ready.
- Backlog.md parses every title and label as written (all 624 tasks).
- Serializer round trip: a CLI edit of every task changed only the
  edited field and `updated_date`, so first touches produce no
  reformatting noise.
- `just check` on the final tree.
- Not checked by me: the browser UI and board by eye, and agent sessions
  in each harness. Those are the evaluation below.

## Numbers

| | Before | After |
| --- | --- | --- |
| Backlog files | 1 (`docs/backlog.md`, 15,387 lines) | 624 task files (median 40 lines, max 164) + 21 milestones |
| Stories | 622 item lines, 620 IDs (2 used twice) | 624 tasks: 584 top-level, 40 subtasks (2 restored parents) |
| Open / done | 118 open | 121 open and 503 done, all in `backlog/tasks/` |
| Dependencies | `Blockers:` prose | 313 tasks with dependencies, 34 prose mentions skipped |
| References rewritten | | 96 files (ADRs, docs, plans, code comments, `Cargo.toml`) |
| Commits touching the backlog | 439 of 919 (48%) | |
| Agent overhead (CLI mode) | | `backlog instructions overview` ~2.2 KB per session; guides 1.8–6.4 KB each |

## Decisions To Review

**IDs**

1. `CSP-NNN`, three digits, numbered by first landing on `main`'s
   first-parent history (author time, UTC). Stories are followed through
   renames: the planning stories were `P0-001`..`003` before
   `PLAN-001`..`003`, and `H-AGENTDECK-001`..`003` became
   `H-AGENTMUX-002`..`004`; both keep their original dates. Reused IDs
   (`P0-001` was reused for the Rust skeleton) get their own later date.
2. Lettered stories became subtasks by letter: `a` is `.01`, `b` is
   `.02` (Backlog.md pads subtasks to two digits).
3. `P8-012` (open) and `P11-011` (done) existed only as lettered
   sub-stories. Both were real entries before being split, so they are
   restored with their last historical titles and dates (`CSP-171`,
   `CSP-448`, label `restored-parent`).
4. `H-MUXPROC-016`/`-017` were each used twice: the 2026-05-23 hook pair
   is `CSP-230`/`CSP-231`, the 2026-06-04 opaque-session-key pair is
   `CSP-358`/`CSP-359`. References inside each story follow its own
   pair; ADR 0059's list of recent resolver stories was read as the
   2026-06-04 pair.
5. Five bare-number shorthands were expanded by hand-written rules (the
   H-RENAME dependency paragraph, two `` `H-WIDG-002` / `003` / `004` ``
   lists, and two H-WT `004a`/`004b` mentions).
6. Left as written because no story defines them: `T8-008`
   (`docs/backlog.md`) and `H-WT-004a` (ADR 0093 twice, backlog twice).
7. Wildcard family names (`H-PIN-*`, `H-REF-*`) stay as written; they now
   correspond to labels. Ranges became `CSP-a..b` when contiguous; two H-WT
   ranges became lists.
8. ADRs and all other docs were rewritten in place, as you asked; commit
   messages keep legacy IDs. ADR 0109 and this directory are excluded.

**Story to task**

9. Title is the first sentence without its period. 17 sentences over 120
   characters were clipped with `…`; the full sentence opens the
   description (`split-report.md` lists them).
10. The description keeps the story's fields verbatim. `Outcome:` notes
    moved to Final Summary with the label dropped and the first letter
    capitalized; qualified labels (`Outcome (design done):`) stay
    verbatim. Tests and manual checks stayed in the description rather
    than becoming acceptance criteria, which would have flattened nested
    lists. Converting them to acceptance criteria is a possible follow-up.
11. Dependencies come from `Blockers:`. IDs after "supersedes", "same as",
    "pairs (naturally) with", "coordinate with", "in parallel with",
    "parallel to", "independent of", "alongside", "before", "overlaps",
    or "planned with" in the same clause, and IDs followed by "if", are
    skipped. Without these rules `doctor` found five cycles. Review the
    **open** entries in `split-report.md`; they decide `--ready`.
12. Labels: the legacy workstream family (`p8`, `h-pin-tui`), plus
    `wont-do` for the three `[~]` stories and `obsolete` for the one `[-]`
    story. Those four are `Done` rather than a custom status, because
    Backlog.md treats the last status as terminal.
13. Priority comes only from the release tiers: P0 high, P1 medium, P2
    low.
14. `ordinal` is the story's position in the old file, so lists keep its
    order.
15. Every task is in `backlog/tasks/`, done ones in the Done column.
    Backlog.md's `completed/` folder is skipped by search, milestone
    progress, and subtask lists. Measured with that layout: "Rust package
    skeleton" found nothing, "sqlite-vec" missed the story itself
    (`CSP-278`), Hardening showed `0/0` instead of `226/294`, and `CSP-533`
    listed one of its two subtasks. The tool's own finalization guide also
    leaves done tasks on the board until someone runs cleanup. The cost is
    that an unfiltered `task list --plain` is 39 KB instead of 7.5 KB,
    though the tool's instructions have agents search or filter by status.
    Command speed is the same (about 0.45 s).
16. One milestone per `##` section (21, `m-0`..`m-20`; Backlog.md numbers
    milestones from 0). The description keeps the section's prose, `###`
    headings, and dependency diagrams, with stories as
    `- **CSP-NNN** Title` lines.
17. `docs/backlog.md` is now a pointer that keeps the old preamble,
    including the `## Later` item "Evaluate Backlog.md migration…", which
    had no ID and which this migration resolves.
18. The `Legacy ID:` line sits after the task's sections, where
    Backlog.md's serializer keeps it.

**Comments**

19. ADR 0100's matcher only knew phase and `H-*` IDs, so eleven Rust
    comments citing `GV-003a`, `REL-003d`, `TEST-002`, or `TEST-006` had
    slipped past it on `main`. After the renumbering the matcher sees
    them, so commit 4 rewrote them to describe behavior (two
    `tests/testing_replay.rs` section headers now say what they replay).
    The HTML explorer's JS and CSS assets carried eleven more `GV-003*`
    comments. They were cleaned in the same commit so that the
    renumbering does not change shipped asset bytes; the scaffold
    snapshot records the new sizes.

**Tooling and harness**

20. Pinned to `c310b70` (the `v1.53.0` tag plus upstream's version bump),
    with its own nixpkgs; `flake-utils` follows ours.
21. `backlog init` output is committed unmodified (CLI mode, `AGENTS.md`
    block, branch scanning and remote fetch off, auto-commit off).
    Conventions live in a `## Work Tracking` section outside the marked
    block.
22. Definition of Done defaults (`just check`; docs and ADRs updated) were
    added to `backlog/config.yml`. They apply to new tasks only. Drop them
    if the extra checklist steps are noise.
23. The block's `<CRITICAL_INSTRUCTION>` makes every session run
    `backlog instructions overview`. I left it as the tool ships it, so
    you can judge the cost. Narrowing it ("only for planning or task-state
    work") would go in Work Tracking, outside the markers.
24. No MCP. If shell quoting trips agents, add it for Claude Code only.
25. Historical audit docs (`docs/adr-audit.md`, `docs/tui-review.md`, and
    similar) keep their "Filed in `docs/backlog.md` § Section" pointers.
    The pointer file explains that sections became milestones.
26. `Cargo.toml` has no `include`/`exclude`, so `cargo package` would ship
    `backlog/`, as it already ships `docs/`. Add `exclude = ["backlog/"]`
    if that matters for publishing.

## Evaluation Checklist

In the worktree, `nix develop`, then:

- `backlog board`, `backlog browser` (127.0.0.1:6420),
  `backlog milestone list`, `backlog task list --ready --plain`,
  `backlog task view CSP-533 --plain`, and `backlog search <words>`.
- Spot-check a few tasks against `git show main:docs/backlog.md`, and the
  open-task dependencies in `split-report.md`.
- In a fresh session of each harness (Claude Code, Codex, opencode)
  started in the worktree: file a story, plan it, and close it. Watch
  whether the agent follows the block and the Work Tracking conventions,
  and how much context the instructions cost.

Harness setup this needs, outside the repo:

- Agents usually run commands as `nix develop --command ...`, which works
  but costs about a second per call. For a bare `backlog` on PATH:
  `nix profile install github:MrLesk/Backlog.md/c310b7087c3d8d618520bfe4b9918e1c8bc468c4#backlog-md`,
  or the same flake in home-manager.
- Claude Code: allow `Bash(backlog:*)` in `.claude/settings.local.json` or
  user settings if auto mode prompts.

## True-Up

Follow `README.md` § True-Up. Also:

- `phase-8-tui-completion` (local and `origin`, 19 commits ahead of
  `main`, last touched 2026-05-30) edits `docs/backlog.md`: land or
  abandon it first.
- Flip ADR 0109 to Accepted and ADR 0009 to "Superseded by ADR 0109", in
  both files and `docs/adr/README.md`.
- Merge with fast-forward or rebase to keep the commit sequence (the
  generated commits are the record of how the migration was done), as a
  deliberate exception to squash-by-default.
- After merging, restart agent sessions, which hold the old `AGENTS.md`,
  and update Claude's project memory note, which points at
  `docs/backlog.md` § Release Readiness.
