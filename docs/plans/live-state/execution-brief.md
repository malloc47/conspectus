# Execution Brief For Milestone m-26 Stories

Read this before starting any story in milestone m-26 (Live Agent State,
CSP-806..CSP-834). It is the shared contract for whoever implements a story.
The story itself (`backlog task view CSP-N --plain`) says what to deliver;
this file says how.

## Where You Work

- Worktree: `/home/jwaggoner/src/oss/conspectus/.claude/worktrees/live-state`,
  branch `feat/live-state`. Run every command from this directory with
  absolute paths. Never touch the main checkout at
  `/home/jwaggoner/src/oss/conspectus` (the operator works there), the
  `tui-framework` worktree, or any other worktree. Never push. Never use bare
  `git stash`.
- Tools come from the dev shell: prefix commands with `nix develop --command`.
- Stories run one at a time, in dependency order, on this one branch. Commit
  only your story's work and leave the tree clean.
- **Never start a task labelled `needs-decision`** (CSP-823..CSP-834). They
  wait for the operator. If a story you are working seems to need one, stop
  and report.

## Read First

1. `CLAUDE.md` (project rules; they override anything here that conflicts).
2. `docs/plans/live-state/README.md` (the plan) and ADRs 0202 to 0206.
   Then ADR 0028 (hook sidecars), 0146 (the hook spool), 0143 (the mutation
   envelope), 0086 (payload privacy), 0140 (presence, the precedent for a
   sparse table), 0083 (snapshot persistence), and 0117 (capabilities).
3. The research note for your harness under `docs/plans/live-state/research/`
   (`01` Claude Code, `02` Codex, `03` OpenCode, `04` Pi) and
   `06-internal-audit.md` section 7 (extension points) and section 8
   (constraints). File and line numbers in the notes were true on 2026-11-12;
   re-find the code, do not trust the numbers.
4. For anything the operator will see: `docs/tui-columns.md` and
   `docs/plans/live-state/tui-options.md`. This milestone's phase 1 renders
   nothing new in the TUI.

## Per-Story Workflow (Backlog.md)

1. `backlog task view CSP-N --plain`. Confirm every dependency is Done. If one
   is not, stop and report; do not work around it.
2. `backlog task edit CSP-N -s "In Progress" -a @claude`. Research the current
   code (earlier stories changed it), then record a plan:
   `backlog task edit CSP-N --plan "1. ..."`.
3. Implement in focused slices. Each commit builds and passes the relevant
   tests. Record progress with `--append-notes`.
4. Before finishing, run `backlog instructions task-finalization`, verify each
   acceptance criterion with evidence, then check the criteria (`--check-ac`)
   and the Definition of Done items (`--check-dod`), write the final summary
   with `--final-summary` (quoted heredoc so backticks survive), and set the
   status to `Done`. Do not run `backlog task complete` and do not move task
   files.
5. Never edit files under `backlog/` by hand; use the `backlog` CLI. Run
   `backlog doctor` if you change dependencies.
6. Scope: deliver what the story says, fully. File out-of-scope work with
   `backlog task create` (labels `live-state` and an `ls-*` area, milestone
   `m-26`) and mention it in your report; do not silently expand the story.
   Where a story leaves a choice open and gives a default, implement the
   default and flag the choice in your report.

## Commits

- Conventional Commit subjects with the task ID, for example
  `feat(model): add the live-state vocabulary (CSP-807)`. Types: `feat`,
  `fix`, `refactor`, `test`, `docs`, `chore`, `build`.
- Body: what and why in a few lines. End every commit message with:
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`
- Include the story's backlog edits in its final commit or a trailing
  `chore(backlog): close CSP-N` commit. Leave no backlog file dirty.
- Keep commits proportional: split a large story by concern (model, behavior,
  tests, docs). Never leave a red test in a commit.

## Checks

Use the scripts in `/home/jwaggoner/.claude/jobs/088c93fc/tmp/ls/`; they
exist because the shell tool refuses inline `env -u` constructs.

- `run-some.sh <cargo test args>` for narrow runs while iterating (for
  example `run-some.sh --lib model::live`). Log: `some.log`.
- `run-tests.sh` for the full suite before the final commit. Log: `test.log`;
  read the `exit:` line and the failures.
- `checks.sh` for `cargo fmt --check`, clippy with warnings denied, rustdoc
  with warnings denied, and `git diff --check`.
- `verify.sh CSP-N` for a quick state report.

**tmux isolation is mandatory.** Some tests start tmux and the host session
runs inside tmux; a stray `kill-server` ends it. Never call `tmux` directly in
a way that can reach the host server, and run tests only through the scripts.

The baseline (before any m-26 code) is in
`/home/jwaggoner/.claude/jobs/088c93fc/tmp/ls/BASELINE.md`. Only failures
beyond it are yours. Known flaky tests ("Text file busy", a `discovery::forge`
timing test, a tmux "server exited unexpectedly"): rerun once before
investigating.

### Disk Hygiene

Every rebuild writes new test binaries under a new hash, and Cargo never
deletes the old ones. If `df -h /` shows under 30 GB free, run
`/home/jwaggoner/.claude/jobs/088c93fc/tmp/ls/cleanup.sh` (it cleans only the
`conspectus` and `tuikit` packages; never run a bare `cargo clean`). Keep
scratch files in `/home/jwaggoner/.claude/jobs/088c93fc/tmp/`, not `/tmp`.

## Rules That Bite For This Milestone

- **Privacy (ADR 0086, Tier 3).** Hook payloads carry prompts, tool inputs and
  outputs, and messages. Parse them in memory against a typed whitelist, keep
  enums, counts, ids, paths, and times, and drop the rest. Every writer story
  has a sentinel test: plant a unique string in every payload-bearing field of
  a fixture and assert it appears nowhere in the store, the spool, logs, or
  any output.
- **The hook must never disturb the harness.** `hook write` prints nothing to
  stdout or stderr, always exits 0, never waits on a daemon for a state
  event, and bounds its own time. Claude Code feeds some hook stdout to the
  model and treats exit 2 as a block.
- **Never touch the operator's real configuration.** Tests and manual checks
  use a scratch `HOME`, `CODEX_HOME`, `PI_CODING_AGENT_DIR`, and
  `XDG_STATE_HOME`. Nothing under the real `~/.claude`, `~/.codex`, `~/.pi`,
  `~/.config/opencode`, or `~/.local/state/conspectus` is read or written by
  a test, and you do not run `claude`, `codex`, `opencode`, or `pi`
  yourself (they use the operator's accounts); fixtures are synthetic and
  labeled so until CSP-822.
- **The model is provider-neutral.** Harness event names stay in adapters and
  plugins; `src/model/live.rs` names no harness.
- **Observation only.** Nothing sends terminal input to an agent (ADR 0028,
  absolute) and nothing the daemon does goes beyond category 2 writes
  (ADR 0143).
- **The graph is sparse.** No entry means no evidence, never "idle".
- **No TUI rendering in phase 1.** CSP-814 carries data onto row models; no
  glyph, column, chip, key, or text is added. `conspectus tui --snapshot`
  frames must be byte-identical before and after for the default views. Build
  with `--features snapshot` for the check.
- Start processes only through `exec::command`; a new program, subcommand,
  socket, signal, or write path needs a `docs/capabilities.toml` entry
  (ADR 0117). `$STATE/conspectus/**` is already a sanctioned write path.
- User-facing text never cites an ADR number, a backlog ID, or a path under
  `docs/` (`src/tui/consistency_tests/text.rs`); code comments explain
  behavior and never cite backlog IDs or narrate history
  (`tests/comment_hygiene.rs`).
- Large unit-test modules (100 lines or more) live in sibling `_tests.rs`
  files (`tests/test_module_hygiene.rs`).
- `csp` is the only short name; docs spell out `conspectus`.
- No new third-party dependency without its own ADR first.
- The graph snapshot format changes bump `FORMAT_VERSION` (ADR 0083) and
  update every full `GraphSnapshot` literal.
- Do not rewrite unrelated files. Preserve behavior the story does not change.
- Update `docs/operations.md`, `docs/library-api.md`, `CHANGELOG.md`, and the
  plan README (section 8 lists existing bugs) when the story changes what an
  operator or library user sees.

## Stop Conditions

- An `EPERM` or "operation not permitted" error: stop at once and report it
  verbatim. The sandbox cannot be widened from inside the session; the
  operator must restart it with the extra path allowed. Do not try other
  paths, copies, or workarounds.
- A dependency story is not Done, or the tree you start from is dirty: stop
  and report.
- Tests that fail for reasons you cannot resolve: do not commit a broken
  state. Report what you tried.
- A decision the story reserves for the operator with no stated default: stop
  and report it as needing input.
- Any need to run a real harness, read the operator's real harness data, or
  widen what Conspectus reads (a screen, a transcript): stop. Those are
  CSP-822 and the parked policy stories.

## Report Back

Finish with a report of at most 250 words: story ID and final status; commit
hashes; the checks you ran and their results (commands and pass or fail);
follow-up tasks filed; and anything that needs the operator's review. Do not
paste diffs or long logs.
