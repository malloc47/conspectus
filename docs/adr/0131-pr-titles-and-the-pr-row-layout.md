# ADR 0131: PR Titles And The PR Row Layout

## Status

Accepted. Amends ADR 0129 (item 2, where the local-branch marker sits)
and ADR 0112 (the title was only known to the preview). Amended by
[ADR 0184](0184-tui-pr-pinned-and-history-rows-come-from-schemas-and-headers-carry-two-cells.md):
a row's `#N` is decided while PRs are filed, a narrow row cuts its
`owner/repo` and keeps the number whole, and the title and branch give
way by the left pane's one narrow-width policy.

## Context

A PR row in the left pane read `⇄ ◌ org/repo#N  state  [draft]  branch
age  (n)`. Four things made it hard to scan:

- Every row repeated `org/repo` under a group that already named the
  repo, and the group named only the local directory (`dek`), not the
  forge repo (`btatnall/dek`).
- Nothing said what a PR was about. The title was only fetched by the
  preview (`gh pr view`), after the PR was selected.
- The columns floated: state, branch, and age shifted with each row's
  text, unlike the other views.
- Most rows ended in `(0)`, the count of attached sessions.

## Decision

1. **`gh pr list` also returns `title`.** It rides in the existing
   call, so it costs no round trip. `ForgePrNode` gains
   `title: Option<String>` (one line: whitespace runs collapse, an empty
   title is none) and `FORMAT_VERSION` goes from 6 to 7, so older
   snapshot files cold-rebuild (ADR 0082). `table prs` gets an opt-in
   `title` column.
2. **The group names the forge repo when it can.** When every PR in a
   `checkout`, `repo`, or unchecked-out group comes from one forge repo,
   the header leads with its `owner/repo` (`btatnall/dek  ~/src/dek`)
   and its rows show only `#N`. A group with several forge repos (a
   fork's origin and upstream) and the `state` grouping keep
   `owner/repo#N` on the rows. Search still matches `owner/repo#N`.
3. **Row order.** Kind glyph, number, local-branch marker, state, age,
   then the content: the local branch (dim, only when there is one),
   then the title (gray). The marker moves from before the number to
   after it, a one-cell glyph so what follows it stays aligned.
4. **Aligned columns, flexible content.** The number, state, and age pad
   to the widest value in the whole tree, not just the visible rows, so
   folding a group never shifts them. The content fills the rest of the
   row and is cut with an ellipsis; the branch is capped at 24 cells so
   it can't crowd out the title. The branch is content rather than a
   column because only tracked PRs have one, and a column would sit
   mostly blank.
5. **An open draft reads `draft`** in the state column, a merged or
   closed draft keeps its state, and **the attached-session count is
   dropped** from PR rows. A PR with attached sessions still shows its
   disclosure arrow and nests them.
6. **Rows are laid out against the width the paragraph really gets**,
   after the scrollbar gutter is reserved, so truncation ellipses are
   not clipped by the scrollbar.

## Consequences

- Titles are as fresh as the forge listing (ADR 0116), not the
  preview's fetch; a retitled PR updates on the next listing.
- The listing payload grows by one short string per PR.
- Machines upgrading cold-rebuild their snapshot once.
- Alignment assumes numbers, ages, and states are short; a 5-digit PR
  number or `120d` age just widens its column.

## Alternatives Considered

- **Fetch titles lazily with the preview.** Rows would show titles only
  for PRs already selected, which reads as a bug.
- **A fixed-width branch column.** Matches the other columns but leaves
  most rows with a wide blank gap before the title.
- **Drop the branch name.** Frees the width, but loses the branch on
  tracked PRs, where it is the most useful.
- **Keep the attached count, hiding only zero.** Still noise on busy
  rows; the nested session rows already show the sessions.
