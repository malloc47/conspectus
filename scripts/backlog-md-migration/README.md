# Backlog.md Migration

One-off tooling that moved `docs/backlog.md` to Backlog.md task files
(ADR 0109). It needs Python 3.11 or newer (standard library only) and
git, and runs from the repository root.

```sh
python3 scripts/backlog-md-migration/migrate.py rewrite   # renumber IDs repo-wide
git commit -am "..."                                      # verify reads the renumbered file from git
python3 scripts/backlog-md-migration/migrate.py split     # docs/backlog.md -> backlog/
python3 scripts/backlog-md-migration/migrate.py verify    # every line accounted for
```

- `rewrite` maps every legacy story ID (`P8-014`) to `CSP-NNN` and
  rewrites references across tracked files, except this directory, the
  ADR that records the migration, and the two ID-matcher tests, which are
  updated by hand. It writes `docs/backlog-legacy-ids.md` and adds a
  `Legacy ID:` line to every story in `docs/backlog.md`. Numbers follow
  the order stories first landed on `main` (first-parent history), so
  re-running on a newer `main` keeps every existing number and appends new
  stories.
- `split` turns the renumbered `docs/backlog.md` into `backlog/tasks/`
  (open and done stories alike) and `backlog/milestones/`, and leaves
  `docs/backlog.md` as a pointer.
- `verify` re-parses the renumbered file from git (`--ref`, default
  `HEAD`) and checks that every story, field, outcome note, section, and
  preamble line appears in the output.

Both generating steps refuse to run on a dirty tree (this directory
excepted) and write a report next to this file for review.

Hand-written rules live at the top of `migrate.py`: bare-number
shorthands (`` (`001`, `002`) `` in the H-RENAME section), the duplicated
`H-MUXPROC-016`/`-017` IDs, and the two restored parents. A rule that
stops matching fails the run instead of being skipped.

## True-up

The generated commits are regenerated, not rebased:

1. Freeze `main`. Land or abandon branches that edit `docs/backlog.md`.
2. Rebase the hand-written commits of this branch onto the new `main`,
   dropping the two generated commits (`docs(backlog): renumber ...` and
   `docs(backlog): split ...`). If `main` took ADR 0109, renumber the
   migration ADR.
3. Re-run `rewrite`, review `rewrite-report.md`, commit; re-run `split`
   and `verify`, review `split-report.md`, commit.
4. Re-apply the tooling commit (`backlog init` output and `AGENTS.md`
   conventions) if it conflicts.
5. Run `just check` and `backlog doctor`, then merge.
