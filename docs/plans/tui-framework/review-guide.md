# Review Guide: The TUI Framework Milestone (m-25)

For the operator reviewing branch `worktree-tui-framework` without having
watched it run. This guide condenses; it does not replace
[`operator-review.md`](operator-review.md), which holds all 437 judgment calls
with the default that shipped and where to change each. Counts of files,
lines, tasks, and records were measured on the branch at `93f1fefa`
(2026-10-31); test counts come from the test logs, frame counts from the
stories' final summaries.

## Summary

- **Delivered:** a workspace crate, `crates/tuikit`, with 36 public modules (theme, glyphs, text, keys, overlay stack, menus, dialogs, forms, palettes, pagers, tree, column schemas, explorer, chrome, test kit), and the Conspectus TUI moved onto it; the host's `src/` shrank by 3,910 lines net (+29,809, -33,719).
- **Framework size:** 33,998 lines of non-test Rust in 140 files, 26,916 lines of unit tests, 3,685 lines of integration tests (the guards and doc sync), and 115 Markdown files (16,009 lines: guide, architecture, house rules, 36 control pages, 71 decision records).
- **Work:** 94 tasks, CSP-710 to CSP-803 (57 planned, 3 split out of CSP-756, 34 follow-ups), 90 Done and 4 open, in 426 commits on `ca668a3` (921 files, +148,993 and -34,593 lines, 8,577 of the added lines in `backlog/`).
- **Tests:** 4,849 passed, 0 failed, 1 ignored at the last full run (CSP-764, `cargo test --workspace --all-targets --all-features`), against 3,424 at the base; the framework pages' doctests run separately under `cargo test -p tuikit`.
- **Records:** 51 ADRs (0150 to 0200), 71 framework decision records, 437 review rows; three dependencies left the build (`ratatui-cheese`, `ratatui-comfy-toaster`, `tui-popup`; the lockfile went from 330 to 320 packages) and none was added.

## Decisions That Need Your Call

Ordered by how soon you would notice. Each row is a default that shipped
without you; "deviation" marks a row where the implementer departed from the
story's text, its design, an ADR, or the brief.

| # | Story | What shipped | Why it might be wrong for you | How to reverse it |
| --- | --- | --- | --- | --- |
| 1 | CSP-797, CSP-760 | Main-screen letters bind plain or with Shift only. Stopped: `Alt` with `a`, `p`, `f`, `!`, `?`, `i`, `o`, `w`, `v`, `r`, `n`, `m`, `u`, `]`, `[`, `/`, `s`, `<`, `>`; the tree's `Alt-h/l/g`; every chord but Shift of `j`, `k`, `G`, `J`, `K` (`Ctrl-j`); `Ctrl+Enter` running the default action. | Your hands or terminal may use them (many terminals send `Ctrl-j` for `Ctrl+Enter`). | Add the chord as a key of its binding in `PROGRAM_KEYS` (`src/tui/keybindings.rs`) and drop the `loose_characters` assertion in `every_character_binds_as_typed_and_every_chord_is_spelled_out`. |
| 2 | CSP-728 | In overlays a letter fires plain or with Shift, never with Ctrl or Alt: `Ctrl-G`/`Alt-G`, `Ctrl-D` (discard in close-down), `Ctrl-Y` (confirm; copy in the value modal), `Alt-Y` (Messages) stopped. `Ctrl-N`/`Ctrl-P` now move every list. | Lost chords. | `KeyMatcher::Char` in `crates/tuikit/src/keys/matcher.rs`, or alias rows in `crates/tuikit/src/keys/standard.rs` and `src/tui/keybindings/overlay.rs`. |
| 3 | CSP-729 | `Esc` steps back one level, `Ctrl-C` closes the whole overlay, `q` closes wherever nothing is typed (menus, checklists, confirmations, Controls, the close-down choice, search options, viewer help). Closing clears the status line; `…: cancelled` and `viewer closed` are gone. | `q` in the close-down choice used to back out, not close. | `q` aliases in `crates/tuikit/src/keys/standard.rs`; `App::modal_closed` in `src/tui/app/overlays.rs`. |
| 4 | CSP-727 | Menus, checklists, palettes, forms, and Controls wrap from last to first; Messages and the tree clamp. A row's marker takes the row's style (an armed menu entry's marker is no longer red). | You may want menus to stop at their ends. | `Ends::Wrap` to `Ends::Clamp` in `ActionMenu::step` (`crates/tuikit/src/action_menu.rs`), and likewise in the other controls. |
| 5 | CSP-756 | Right pane: `PgUp`/`PgDn` page, `h`/`l` and the arrows fold `Other`, `Esc` backs out of a drill, chords of `j k g G h l` do nothing. Left `Backspace` only returns from a reveal. `Action::OpenValueModal` was removed, where the explorer design's section 9.3 contradicted its section 1.5. | Changes settled gestures on both panes. | `EXPLORER_KEYS` (`crates/tuikit/src/explorer/keys.rs`), `RIGHT_PANE_KEYS` (`src/tui/keybindings.rs`), `App::left_back` (`src/tui/app/explorer_nav.rs`). |
| 6 | CSP-710 | `e` removed from the left pane only (it toggled an invisible flag) and kept on the right pane as the `Other` toggle; the story called it inert everywhere (deviation). | You may have wanted it gone. | Remove the right-pane binding, decision 8 in `docs/design.md`, and the help text. |
| 7 | CSP-736 | `Enter` submits a form from any field, so the new tmux session form creates from its name field (`Enter create`, was `Enter next/commit`). The framework has no "advance" policy to opt back into. | One `Enter` no longer shows you the cwd before creating. | `NEW_MUX_KEYS` (`src/tui/keybindings/overlay.rs`) plus a new advance policy in `Form::key` (`crates/tuikit/src/form.rs`). |
| 8 | CSP-721 | Five muted keys merge into `muted` (dim: `placeholder`, `divider`, `mux_unmuxed`) and `inert` (dark gray: `secondary_text`, `ghost`); conflicting old keys merge by `ALIASES` order with a warning; `accent` and `panel_focus_accent` feed each other; `[tui.theme.harness]` now beats a flat `harness_<name>`. Aider's theme rows stay because the plan's dependency on CSP-700 was dropped (deviation). | A theme that set only `placeholder` now also restyles rules and `∘`; harness precedence flipped. | `ALIASES` and `ROLES` in `src/tui/theme.rs`; `default_aliases` in `crates/tuikit/src/theme.rs`; `merge_tui_theme` in `src/config.rs`. |
| 9 | CSP-757 | The focused pane's whole border takes `panel_focus_accent` (cyan, follows `accent`), also under an open modal; titles keep their own looks. Of 584 frames checked, 288 differ only by this tint. | The most visible change on the main screen. | The focused branch in `PaneFrame::render` (`crates/tuikit/src/frame/pane.rs`); for the modal case, `focused(... && !app.has_modal())` in `draw_right_panel` and a separate frame-focus flag on `TreeView`. |
| 10 | CSP-775 | The left tree draws the `►` gutter on its cursor row, so every row starts two cells right; the cells come from the fill first (at 40 columns Mux drops its ages). A row listed twice highlights only the copy under the cursor. 17 insta snapshots changed. | Two columns fewer in a narrow pane. | `TreeView::row_line` and `RowCtx::row` (`crates/tuikit/src/tree/view.rs`), `MUX_NAME_STEPS`; then `just tui-columns`. |
| 11 | CSP-731 | Severity by what happened: hints and confirmations green, refusals and validation yellow, failures red; info and success share green; toasts last 1.5 s, 3 s (warning), 5 s (error); a long path in a toast breaks mid-word, not at hyphens or slashes. | Info reads like success. | No code: `[tui.theme] info = "cyan"`. Code: the `info` default in `crates/tuikit/src/theme.rs`; `Severity::ttl` in `crates/tuikit/src/feedback.rs`. |
| 12 | CSP-759 | The status bar is generated: under an overlay it repeats that overlay's footer (keys show twice) without the `[left]`/`[right]` tag; the left pane shows `↑/↓ move · ←/→ fold`, not `j`/`k`; `Enter default` is gone; pin hints are short (`Enter rebuild`); a key that cannot act is dimmed with its reason. | Duplication; `j`/`k` users lose the reminder. | `status_hints` and `pin_enter` in `src/tui/ui/header.rs`; binding labels in `PROGRAM_KEYS`. |
| 13 | CSP-781, CSP-784 | One modal footer: `muted` under a blank row, keys that leave listed last; when short, the blank row goes first, then the first key rows spill after the body. At 40x10 Clean up lists two items (was three) and content search one result. | Dimmer footers; fewer body rows on small terminals. | `ModalFooter::style` and `ModalFooter::fit` in `crates/tuikit/src/frame/footer.rs`. |
| 14 | CSP-734 | Prompts carry their keys on the bottom border and a refusal on an error line in red, as forms do, though ADR 0163 classes a validation message as a warning, yellow on the status line and in toasts (deviation). | Two colors for one kind of message. | The style in `Body::render` (`crates/tuikit/src/text_prompt.rs`) and the forms' error line (`crates/tuikit/src/form/body.rs`). |
| 15 | CSP-726, CSP-742 | Five modal sizes and one margin (two columns, one row): menus 64 columns (the row menu lost 8), forms 80, the picker takes the search palette's size; on a 24-row terminal a long menu shows two fewer rows. | Less room on small terminals. | `SIDE_MARGIN`, `END_MARGIN`, and the sizes in `crates/tuikit/src/frame/size.rs`; a modal's `ModalFrame::new` call. |
| 16 | CSP-714 | Draft PRs: state beats draft. A closed or merged draft shows only its closed or merged glyph; an open draft keeps its draft look. The story reserved this rule for your confirmation. | You may want closed drafts marked. | `PrStatus` in `src/tui/rows/pr_status.rs` and the rule in `docs/tui-columns.md`. |
| 17 | CSP-711, CSP-769 | The hidden Union and Forks views are gone (`--view union` and `--view forks` fail). No view lists a fork, so quick select reads `not listed` and **Reveal in its view** shows only where that view lists the entity (ADR 0198; CSP-769 left the choice to you). | Forks are findable but not openable on the left. | The CSP-769 row's options: filter results in `SearchOverlayState`, or the kind-only rule in `App::reveal_view` (`src/tui/app/listing.rs`). |
| 18 | CSP-716 | Fixture mode (`tui --fixture`, `dev scenario tui`, `just demo`) was kept and fixed, not removed: live keys in its menus and forms, and a pick that would write or launch says it isn't available in this mode. | It is the third key-dispatch path CSP-802 targets. | Remove `run_from_fixture`, `run_static`, the `demo` recipe, and their docs (`src/tui/runtime.rs`). |
| 19 | CSP-737 | One argv tokenizer with POSIX double-quote rules for previews, history, rebuild, and `pin edit`; typed argv is strict, so `pin edit --set argv=…` and the pin edit form refuse an unclosed quote or a trailing backslash. A CLI change. | Scripts passing unfinished argv now fail. | `tokenize` in `src/argv.rs`; `parse_override` in `src/pin_ops.rs` (call `split_command_line` there). |
| 20 | CSP-723, CSP-774, CSP-800, CSP-801 | The stored snippet is 160 cells cut by the TUI's `fit_snippet`, which changes `snippet.text` in `conspectus search --json` (shape unchanged) and the CLI table's crop; a cut lands inside a word only beside a token (a wide character, a path or URL separator, or over 24 cells). | Machine-readable output text changed; the 24-cell threshold is a judgment call. | `snippet_for` in `src/viewer/search.rs`; `crop_around` from history in `src/cli/search.rs`; `WORD_CELLS` and `is_token` in `crates/tuikit/src/text/snippet.rs`. |
| 21 | CSP-717, CSP-763, CSP-764, CSP-785 | New pre-main checks: `--workspace` on every cargo check and rustdoc with `-D warnings` (CSP-717), `just tuikit-features` (in `just check`), and `just separation-rehearsal --self-test` (about 4.5 minutes locally; a CI job on every push and PR). CSP-717 and CSP-764 wrote them into `AGENTS.md`'s Pre-Main Invariants, and CSP-764 added three more bullets (+64, -7 lines in all); the agents of CSP-763 and CSP-785 had declined to edit the instruction file. | Instruction-file changes made unattended, and a per-merge cost. | `git diff ca668a3..HEAD -- AGENTS.md`; the job in `.github/workflows/ci.yml` (path-filter it or drop `--self-test`). |
| 22 | CSP-766, CSP-541 | `tuikit` is `publish = false` (ADR 0150) and the package depends on it by path, so `cargo publish` cannot pass. CSP-104 now depends on CSP-541, which must choose: publish `tuikit` too (amending ADR 0150), fold it into the package, or release from git only. | Blocks the crates.io release path. | Decide in CSP-541; `backlog task edit CSP-104 --dep CSP-103` undoes the dependency. |
| 23 | CSP-765, CSP-802 | ADR 0200: the runtime (layer 3) stays in Conspectus, LATER, until a second program commits to `tuikit` with its own loop and needs a terminal hand-off, background work, or a headless mode. 11.5% of the runtime's lines are generic. CSP-802 (fold `Action` into `Msg`, removing 257 lines of three drifting dispatchers) was filed on its own merits. | You may want go, or no-go for good. | Go: file the nine stories in section 4.1 of [`runtime-evaluation.md`](runtime-evaluation.md) and supersede ADR 0200. CSP-802: keep, close, or move to Draft. |
| 24 | CSP-718, CSP-726, CSP-731, CSP-741 | The three dependencies became in-tree `ModalFrame`, `Toasts`, and `Checklist` (ADR 0152). `toml` became a framework dev-dependency, and the implementer amended ADR 0152 item 6 so a lockfile crate used only by the framework's tests is not a new dependency. | The amendment loosens your "no new dependency" rule. | Parse the manifest by hand in `crates/tuikit/tests/independence.rs` and drop the sentence added to item 6; re-adding a crate means reverting its control. |
| 25 | CSP-793 | A non-bare repo's breadcrumb hop is its directory (`project`, was `.git`) and the id suffix is four characters of its `id` row. Flagged by the implementer: a checkout-and-repo chain reads `◇ project·d811 › ◆ project·c955`, and at 40 columns the last hop is cut. | Noisier breadcrumbs. | `hop_suffix` and `short_node_label` in `src/tui/explorer.rs`, or compare glyph and tag in `Breadcrumb::repeated` (`crates/tuikit/src/breadcrumb.rs`). |

Other rows that depart from a story, a design, or a house rule, with less
impact: CSP-725 kept chip labels left-aligned where the story asked for
centered; CSP-742 kept a scroll window instead of the sketched bottom-pinned
one; CSP-743 kept quick select's own cursor rule after an edit; CSP-744 used
`Stay` where its story said `Commit` and added `Outcome::Back` to the overlay
contract; CSP-747 left the Messages record pane without the `Ctrl-D`/`Ctrl-U`
house rule 12 asks for; CSP-800 could not keep spaced text identical as asked,
and CSP-801 narrowed it back.

## Themes

All 437 rows of [`operator-review.md`](operator-review.md), grouped by the
story that wrote them, with a few representative items each. Counts include
the rows already listed above, so every row is reachable from here.

**Key conventions and help** (42 rows: CSP-710, 728, 729, 730, 760, 796, 797, 798)
- Help pages are generated from the key tables and list every bound key, aliases included (CSP-730, CSP-760).
- One scoped keymap; two keys of one scope that one event fires fail the build (CSP-760).
- `key_label` is the one key notation: `Ctrl-R`, `S-Tab`, `↑`, `Space` (CSP-728).
- Messages and empty-state hints name keys from the keymap, and a scan flags hand-typed keys (CSP-796, CSP-798).

**Theme, color, and glyphs** (17 rows: CSP-714, 721, 722, 773)
- New tokens with the old defaults (`field`, `field_active`, `field_cursor`, `surface`, `info`); `fg on bg` and `light_gray` parse (CSP-721).
- `▶` and the Braille spinner are the two glyph exceptions kept by preference (CSP-722).
- The vetted-glyph scan covers all of `src` and the framework, with 18 new registrations (CSP-722).
- A host guard refuses raw colors and ratatui's `Stylize` in `src/tui` and `src/viewer` (CSP-773).

**Text, scrolling, chips, and selection** (33 rows: CSP-723, 724, 725, 727, 779)
- One wrap rule; key-hint footers wrap only between hints (CSP-723).
- The transcript viewer keeps the blank lines between Markdown blocks, so transcripts read taller (CSP-723).
- TUI ages stop at days while CLI tables keep weeks; `scanned just now` reads `scanned 0s ago` (CSP-725).
- A body under five rows draws no scrollbar and lays out at full width (CSP-779).

**Rows, columns, and narrow widths** (35 rows: CSP-750, 751, 752, 775, 786)
- Columns give way by declared priority; a fill needs 7 cells; labels narrow to 8 and then leave (CSP-751).
- Fixed drop orders for session, mux, PR, Pinned, and History rows (CSP-751, CSP-752).
- Nested group headers line up with their parents (CSP-752, CSP-772).
- An overflowing tree keeps its layout and its fill gives the scrollbar a cell (CSP-786).

**Chrome: frames, header, tab strip** (22 rows: CSP-716, 726, 757, 758)
- One modal title form, plain text in the border's accent (CSP-726).
- The header gives way in a fixed order; freshness outlasts every count but sessions (CSP-758).
- The tab strip drops inactive names from its end and ends with ` …` (CSP-716, CSP-758).
- Preview placeholders recede (dim) like the panes' empty states (CSP-758).

**Feedback, footers, and the status bar** (35 rows: CSP-731, 759, 781, 784, 795)
- A failed clipboard write is an error toast; it used to say `copied` regardless (CSP-731).
- An overlay's keys outrank the view's settings on a narrow bar (CSP-759).
- A status message wider than the bar ends with `…` (CSP-759).
- Framework empty notes draw through `EmptyState`; the empty Messages note recedes (CSP-795).

**Search and quick select** (28 rows: CSP-711, 742, 743, 753, 769, 774, 782, 787, 800, 801)
- Palettes put the cursor on the first result after an edit; quick select keeps its own rule (CSP-742, CSP-743).
- Hits read `turn N · kind` in the modal and the Search view; the role word (`you`, `ai`) is gone (CSP-753).
- `PgUp`/`PgDn` page the search palettes without being named in the footers (CSP-782).
- Quick select's entity rows come from a column schema with two new width rules (CSP-787).

**Forms, prompts, and text entry** (55 rows: CSP-715, 733, 734, 735, 736, 737, 738, 778, 799)
- A multi-line paste into a one-line field is flattened; a paste outside a text field is ignored (CSP-715).
- One caret (`field_cursor`) and one windowing rule; `Ctrl-U` in a find prompt clears the query (CSP-733).
- Path completion only extends the typed text, offers directories only, and marks advisory paths `▲` (CSP-735).
- Both pin rename UIs stay and share one name check; `pin rename` checks before touching tmux (CSP-738, CSP-778, CSP-799).
- A short form keeps its focused row, error line, and footer before notes and other rows (CSP-736, CSP-738).

**Menus, dialogs, checklists, and settings** (35 rows: CSP-739, 740, 741, 744, 780)
- A shortcut column appears only for keys that do the same as on the list (`p` Pin, `n`/`N` mux) (CSP-739).
- A follow-up question is a step: `Esc` closes the step, `Ctrl-C` or `q` the whole flow (CSP-740).
- Checklists take `a` (all) and `n` (none) and show their count on the border (CSP-741).
- Controls' editors open as steps and the panel stays open after an answer (CSP-744).

**Viewers and pagers** (40 rows: CSP-745, 746, 747, 748, 749)
- The pane viewer wraps long lines under their number; `w` cuts them instead (CSP-745).
- A find lands one row below the top, and `G` sticks to the end in every pager (CSP-745).
- The value modal's footer order changed and it gained a scrollbar (CSP-746).
- Transcript colors moved to `accent` and a new `transcript_compaction` role (CSP-748).
- Two Preview scroll behaviors changed that one-frame snapshots cannot show (CSP-749).

**Explorer and right pane** (47 rows: CSP-754, 755, 756, 768, 788, 789, 790, 791, 792, 793, 794)
- Detail rows are a property grid; `Enter` on a placeholder copies nothing (CSP-754).
- A refresh keeps the drill and the Preview's place (CSP-789, CSP-792).
- Breadcrumb hops give way oldest first and the last is cut with `…` (CSP-790).
- The `Related` rule cuts its summary instead of wrapping at 40 columns (CSP-794).
- Hints for an ambiguous group's `a` and an unresolved row's `Enter` now say what is true (CSP-768, CSP-788).

**Tests, guards, and CI** (18 rows: CSP-718, 732, 763, 785)
- `testkit` is a cargo feature; the framework depends on itself by path to test with it (CSP-732).
- Style-aware goldens are text rows plus a legend, accepted with `TUIKIT_ACCEPT_GOLDENS=1` (CSP-732).
- Snapshot mode runs overlay messages through the reducer and skips only effects that reach outside (CSP-732).
- The vocabulary guard skips lock files by name; the export script refuses dirty or shallow trees (CSP-763).

**Documentation and decision records** (19 rows: CSP-761, 762, 764)
- Every `rust` block in the framework's pages compiles as a doctest (`src/doc_pages.rs`) (CSP-761).
- Doc sync holds the template sections, feature rows, house-rule numbers, links, and module map (CSP-761).
- 64 host ADRs are marked framework-owned in whole or part, and the log restates them (CSP-762).
- `docs/operations.md` gained "What changed in the TUI"; changelog bullets were corrected in place (CSP-764).

**Close-out: runtime and backlog** (11 rows: CSP-765, 766)
- The runtime's lines were counted generic, domain, or wiring; the seam's three choices have defaults (CSP-765).
- CSP-435 and CSP-455.01 closed, CSP-432 retargeted, CSP-430 retitled (CSP-766).
- CSP-193 and CSP-098 may already be delivered, and CSP-191 closes with CSP-270 (CSP-766).

## Open Items

| Task | Priority | What it is | What it needs |
| --- | --- | --- | --- |
| CSP-770 | Low | Fake-binary tests fail now and then with "Text file busy". | One helper that writes, syncs, and closes a fake executable before any exec; 20 clean full runs. |
| CSP-776 | Low | Overlay footers name keys that cannot act (`y copy` on an empty Messages log). | Availability set in the host's tables, every footer drawn through `tuikit::keys::Footer`. |
| CSP-802 | Low | Fold the keymap's `Action` into `Msg` so live, fixture, and snapshot modes share one dispatch. | Your keep, close, or Draft call (row 23), then the work. |
| CSP-803 | Medium | The TUI ignores `--color never` and `NO_COLOR` and never downgrades colors to the terminal's depth. | Host detection plus `tuikit::theme` functions to strip and downgrade; docs and roadmap. |

## Merge Notes

- **Branch and base.** `worktree-tui-framework` holds 426 milestone commits plus this guide's, on `ca668a3`. `git rev-parse --short main` gives `ca668a35` and `git log --oneline ca668a3..main | wc -l` gives 0, so `git merge --ff-only worktree-tui-framework` from the main checkout works while `main` is unchanged. `origin/worktree-tui-framework` points at `ca668a35`: nothing of the milestone was pushed.
- **Squash or fast-forward.** `AGENTS.md` prefers squash merges, but at least 16 branch commits are cited by hash in the docs and task notes (CSP-764's reversal names `85c2b7c2`, CSP-756's `1d2e585d` and `334d1d37`). After a squash, keep the branch or tag it so those hashes still resolve.
- **If `main` moves.** Task IDs: `main` ends at CSP-709 and the branch adds CSP-710 to CSP-803 and milestone m-25; `check_active_branches` is off, so a task filed on `main` meanwhile reuses an ID. After rebasing, run `backlog doctor`, read its repair preview, apply it with `backlog doctor --fix`, then fix the references it reports and the files outside `backlog/` that cite these IDs: 39 ADRs, 15 plan files (this guide included), `docs/design.md`, and `docs/tui-architecture-review.md` (`git grep -lE 'CSP-(7[1-9][0-9]|80[0-3])\b' -- . ':!backlog'` also lists a sample string in `crates/tuikit/tests/independence.rs`, which is not a reference). Commit messages keep the old IDs, so record the old-to-new mapping in the squash message or a follow-up commit.
- **ADR numbers.** `main` ends at 0149 and the branch adds 0150 to 0200. If `main` adds an ADR, move the branch's block above `main`'s last, highest number first so no rename lands on a file not yet moved: `git mv` each file, update `docs/adr/README.md`, and every `ADR NNNN`, `adr/NNNN-`, and `[NNNN]` citation (186 files with this guide, 70 under `crates/tuikit`, mostly the decision log's Origin sections). `crates/tuikit/tests/doc_sync.rs` fails until the log and the index agree again.
- **Instruction file.** `CLAUDE.md` is a symlink to `AGENTS.md`; the milestone edited `AGENTS.md` (row 21).
- **Worktree.** `/home/jwaggoner/src/oss/conspectus/.claude/worktrees/tui-framework`; the main checkout was not touched.
- **Known flakes.** CSP-770 (ETXTBSY in the forge tests and `rebuild::script_tests`) and the `discovery::forge` timing test `a_cancelled_hosts_lookup_returns_promptly_with_no_answer`; both pass on a rerun.
- **Exploring.** Start the framework docs at `crates/tuikit/README.md` and `crates/tuikit/docs/guide.md`, and the operator-visible changes at [What changed in the TUI](../../operations.md#what-changed-in-the-tui) and `CHANGELOG.md` `[Unreleased]`. Some tests start tmux: run the suite with `TMUX` unset and a private `TMUX_TMPDIR`, as `/tmp/tuifw-baseline/run-tests.sh` does, or a stray `kill-server` can end your session.

```sh
nix develop
snap='cargo run --features snapshot -- tui --snapshot --snapshot-fixture tests/fixtures/showcase.json'
$snap --snapshot-width 100 --snapshot-height 30                  # whole screen
$snap --snapshot-keys '<Tab>' --snapshot-pane right              # focus tint
$snap --snapshot-keys '.'                                        # row menu
$snap --snapshot-width 40 --snapshot-height 10 --snapshot-keys '.'  # narrow
$snap --snapshot-keys '?'                                        # generated help
$snap --snapshot-keys '<Tab>jjj' --snapshot-pane status          # right-pane hints
just demo                                 # interactive, on the same fixture
just tuikit-features                      # the framework alone, features off and on
just separation-rehearsal --self-test     # export and check alone (slow)
cargo test --workspace --all-targets --all-features   # TMUX unset first
```

## How The Work Was Verified

[`execution-brief.md`](execution-brief.md) required, before each story's
final commit, `cargo fmt --check`, `cargo clippy --workspace --all-targets
--all-features -D warnings`, the full suite through the tmux-isolated runner,
`cargo doc` with `-D warnings`, and `git diff --check`. All 90 Done tasks
record a whitespace check and 85 a clippy run; the five without are the
design and docs-only stories (CSP-755, CSP-765, CSP-766) and two closed by
other stories (CSP-771, CSP-772). CSP-765, which touched the decision log,
also ran the `doc_sync` and `independence` tests. Each story with visible
changes lists them in its final summary as regressions and consistency wins.

- **Frames.** Visible changes were compared before and after with `--snapshot` on the showcase fixture at the widths and focus states the story named, with counts in the final summary (CSP-757: 584 frames; CSP-751: 1,095 left-pane frames). The 171 frame sets sit in `/tmp/tuifw-baseline/`, a tmpfs that does not survive a reboot; none is in the repository. Beyond the tmux end-to-end tests in the suite, CSP-715 and CSP-716 also ran the fixture TUI in a pseudo-terminal.
- **Guards in the suite.** Framework independence and vocabulary (CSP-718), raw styles, glyph literals, the documentation lint (CSP-720), and doc sync (CSP-761, with the decision log from CSP-762); on the host, the source scans in `tests/`, `src/tui/consistency_tests/`, and the conformance suite.
- **Framework alone.** `just tuikit-features` from CSP-785 on; `just separation-rehearsal --self-test` from CSP-763 on, last run in CSP-764 (passed, both planted leaks caught; 4m 40s from a fresh clone in CSP-763).
- **Last full run.** CSP-764, 4,849 passed. Only documentation changed after it.

Caveats:

- Frames come from snapshot mode: one frame after scripted keys on a fixture world, with no live tmux and the Preview loaded only for the last frame.
- Snapshot mode skips effects that reach outside (copy, attach, launch, store writes) and shows only a note, so frames say nothing about them. Several behaviors rest on tests alone, not frames: refresh keeping the drill and the Preview's place (CSP-789, CSP-792; `r` in snapshot mode runs real discovery), the Preview's scroll changes (CSP-749), pin rename ordering (CSP-778 with a fake tmux in executor tests, CSP-799 with a fake `tmux` on `PATH`), and a live mux rename (CSP-734: the fixture has no live mux).
- No CI has run on these commits: `origin/worktree-tui-framework` is at the base, so the new framework steps and the Separation rehearsal job have never run on GitHub.
- `scripts/export-tuikit.sh` passed shellcheck 0.11 once (CSP-763, through `nix run`); shellcheck is not in the dev shell or CI.
- The framework pages' doctests run under `cargo test -p tuikit` (in `just tuikit-features` and CI), not in the suite's `--all-targets` count.
