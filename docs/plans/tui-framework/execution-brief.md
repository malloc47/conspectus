# Execution Brief For Milestone m-25 Stories

Read this before starting any story in milestone m-25 (TUI Framework
Extraction, CSP-710..CSP-766). It is the shared contract for whoever
implements a story. The story itself (`backlog task view CSP-N --plain`) says
what to deliver; this file says how.

## Where You Work

- Worktree: `/home/jwaggoner/src/oss/conspectus/.claude/worktrees/tui-framework`,
  branch `worktree-tui-framework`. Run every command from this directory with
  absolute paths. Never touch the main checkout at
  `/home/jwaggoner/src/oss/conspectus` (the operator works there) or any other
  worktree. Never push. Never use bare `git stash`.
- Tools come from the dev shell: prefix commands with `nix develop --command`.
- Stories run one at a time, in dependency order, on this one branch. Commit
  only your story's work and leave the tree clean.

## Read First

1. `CLAUDE.md` (project rules; they override anything here that conflicts).
2. ADRs 0150, 0151, and 0152 in `docs/adr/`: the boundary, the 18 house rules,
   and the dependency decisions.
3. `docs/plans/tui-framework/control-set.md` (what each control absorbs and
   what stays in Conspectus) and the inventory slice files the story cites
   under `docs/plans/tui-framework/inventory/` (they hold `file:line`
   evidence from commit `ca668a3`; line numbers drift as earlier stories
   land, so re-find the code, do not trust the numbers).
4. `docs/plans/tui-framework/workspace-impact.md` for build, CI, and
   source-scanning-test consequences.
5. The framework's own `crates/tuikit/docs/` once it exists
   (CSP-717/CSP-720): architecture, contribution rules, and the control page
   template.

## Per-Story Workflow (Backlog.md)

1. `backlog task view CSP-N --plain`. Confirm every dependency is Done. If one
   is not, stop and report; do not work around it.
2. `backlog task edit CSP-N -s "In Progress" -a @claude`. Research the current
   code (earlier stories changed it), then record a plan:
   `backlog task edit CSP-N --plan "1. ..."`.
3. Implement in focused slices. Each commit builds and passes the relevant
   tests. Record progress with `--append-notes`.
4. Before finishing, run `backlog instructions task-finalization`, verify each
   acceptance criterion with evidence, then:
   - check the criteria (`--check-ac`) and the Definition of Done items
     (`--check-dod`);
   - write the final summary with `--final-summary` (use a quoted heredoc so
     backticks survive). For any story with visible TUI changes, the summary
     has two headed lists, **Regressions** and **Consistency wins**, naming
     every visible change; if there are none, say so explicitly;
   - set the status to `Done`. Do not run `backlog task complete` and do not
     move task files: tasks stay on the board.
5. Never edit files under `backlog/` by hand; use the `backlog` CLI so
   metadata stays consistent. Run `backlog doctor` if you change dependencies.
6. Scope: deliver what the story says, fully. If you find out-of-scope work,
   file it with `backlog task create` (label `tui-framework`, milestone
   `m-25` if it belongs to the extraction) and mention it in your report; do
   not silently expand the story. If the story says a choice needs the
   operator's confirmation in review, implement the proposed default, record
   the choice in the final summary, and flag it in your report.

## Commits

- Conventional Commit subjects with the task ID, for example
  `refactor(tui): merge truncation helpers into tuikit::text (CSP-723)`.
  Types in use: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `build`.
- Body: what and why in a few lines. End every commit message with:
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`
- Include the story's backlog edits (status, plan, notes, checked criteria,
  final summary) in the story's final commit, or in a trailing
  `chore(backlog): close CSP-N` commit. Do not leave backlog files dirty.
- Keep commits proportional: split a large story into reviewable commits by
  concern (move, behavior change, tests, docs).

## Checks

Run from the worktree root. While iterating, run the narrowest relevant tests.
Before the final commit of a story, run all of these and fix what they report:

- `nix develop --command cargo fmt --all -- --check`
- `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`
  (before CSP-717 lands there is no workspace: use the commands without
  `--workspace`)
- the full test suite via the isolated runner below
- `nix develop --command cargo doc --workspace --no-deps` with
  `RUSTDOCFLAGS='-D warnings'` once the workspace exists
- `git diff --check`

**tmux isolation is mandatory.** Some tests start tmux; the host session runs
inside tmux and a stray `kill-server` ends it. Run the suite only through
`/tmp/tuifw-baseline/run-tests.sh` (it unsets `TMUX`, sets a private
`TMUX_TMPDIR`, and logs to `/tmp/tuifw-baseline/test.log`; read the log's tail
and the `exit:` line), or copy its first lines into any other script you
write. Never call `tmux` directly. The script file form is required: the
shell tool refuses inline `env -u` constructs.

If the baseline had failures, they are listed in
`/tmp/tuifw-baseline/BASELINE.md`; only failures beyond those are yours.

## TUI Changes And Verification

- Visible output is validated with `conspectus tui --snapshot` (dev-only; build
  with `--features snapshot`, for example
  `cargo run --features snapshot -- tui --snapshot --snapshot-keys "j<Tab>jj"`
  with `--snapshot-pane left|right|header|status`). Check normal and narrow
  widths, and focused and unfocused states, as the story names. Never ask the
  operator for a screenshot.
- Behavior changes that follow the house rules (ADR 0151) are intended. Update
  the tests and snapshots that encode the old behavior deliberately, and say so
  in the final summary. A test that fails for any other reason is a bug in
  your change.
- `docs/tui-columns.md` and its tests must agree with the code in the same
  change; regenerate frames with `just tui-columns` and edit prose by hand.
- Operator-facing changes (config keys, CLI values, keys, behaviors) go in
  `docs/operations.md` and `CHANGELOG.md` when they would surprise an
  operator. The closing documentation story (CSP-764) sweeps what remains.

## Project Rules That Bite

- User-facing text never cites an ADR number, a backlog ID, or a design doc
  path; `src/tui/consistency_tests/text.rs` enforces it.
- Code comments explain behavior and rationale and never cite backlog IDs or
  narrate history (`tests/comment_hygiene.rs`); "backlog `ID`" is allowed only
  to point at open work.
- Start processes only through `exec::command`; a new program, socket, signal,
  or write path needs a `docs/capabilities.toml` entry (ADR 0117). The
  framework crate starts no processes and writes no files.
- Large unit-test modules (100 lines or more) live in sibling `_tests.rs`
  files (`tests/test_module_hygiene.rs`).
- `csp` is the only short name; docs spell out `conspectus`.
- No new third-party dependency. Removing the three shims is sanctioned by
  ADR 0152; anything else needs its own ADR first.
- Do not rewrite unrelated files. Preserve behavior the story does not change.

## Framework Independence (once `crates/tuikit` exists)

`tuikit` imports nothing from `conspectus`, names no Conspectus domain word
(session, mux, harness, pin, worktree, PR, graph node), spawns no process, and
writes no file. Domain meaning enters through type parameters (`M`, `Cx`,
`Id`, `R`), closures, traits, and the named-style registry. The guard tests
(CSP-718) enforce this; if one fails, fix the design, not the allowlist,
unless the story says otherwise.

The guards live in `crates/tuikit/tests/independence.rs`. Its module docs
describe each rule, how matching works, and how to request a reviewed
allowlist entry; the word list and the dependency allowlist are constants in
that file. Two things to know before writing framework code:

- The vocabulary guard also flags the test-rig sense of "harness". Call such
  helpers a driver, checker, or suite (CSP-732 names a "conformance harness"
  in its title; the framework's own name for it must avoid the word).
- Everything in the crate directory is scanned except that one file: source,
  docs, tests, the manifest, and any README. Docs therefore cannot quote the
  word list; they point at the constant instead.

### Words The Framework Cannot Use

The vocabulary guard (`crates/tuikit/tests/independence.rs`, `DOMAIN_TERMS`)
fails on whole-word, case-insensitive hits, including plurals and the
snake/camel forms, in `tuikit` source, docs, tests, and manifest. Read the
list, then write around it. Common traps: "pinned footer" or "pinned row" (say
"fixed" or "sticky"), "conformance harness" or "test harness" (say
"conformance suite" or "test rig"), "transcript" (say "document" or
"content"), "session" (say "run" or "context"), "repo", "checkout", "PR",
"forge", "mux", "tmux". The Conspectus-side adapter, in `src/`, may use any
of them.

## Stop Conditions

- An `EPERM` or "operation not permitted" error: stop at once and report it
  verbatim. The sandbox cannot be widened from inside the session; the
  operator must restart it with the extra path allowed. Do not try other
  paths, copies, or workarounds.
- A dependency story is not Done, or the tree you start from is dirty: stop
  and report.
- Tests that fail for reasons you cannot resolve: do not commit a broken
  state. Report what you tried.
- A decision that the story reserves for the operator and has no stated
  default: stop and report it as needing input.

## Report Back

Finish with a report of at most 250 words: story ID and final status; commit
hashes; the checks you ran and their results (commands and pass or fail);
the visible changes in two lists (Regressions, Consistency wins); follow-up
tasks filed; and anything that needs the operator's review. Do not paste
diffs or long logs.
