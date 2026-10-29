# Changelog

All notable changes to Conspectus are tracked here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/) from 0.1.0
onward (pre-1.0, so minor versions may break).

## [Unreleased]

Targeted for 0.2.0, after 0.1.0 is tagged. History, rebuild, and pins
(ADRs 0140 to 0144) rework how pinning works.

### Added

- **History.** Conspectus remembers every mux, checkout, repo, and PR it
  sees under `$XDG_STATE_HOME/conspectus/history/`, so an entity that
  disappears stays visible as *gone* (decided by a targeted check, never
  by absence from a scan). `conspectus history list|show|reap|forget`
  inspects and prunes it; `reap` honors `[history.reap_after]` and never
  removes a pinned or Unknown record.
- **Pins.** `conspectus pin
  add|edit|rename|retarget|refresh|move|rm|list|show|migrate`. A pin is
  a named pin in `$XDG_DATA_HOME/conspectus/pins.toml` that points at a
  mux, session, worktree, repo, or PR (live or gone), with overrides
  layered over the live graph, history, and a snapshot. Pinning writes
  only that file. Renaming a live mux pin renames its tmux session with
  it, and `mux new|launch --pin [--no-launch]` pins a new mux or plans
  one.
- **Rebuild.** `conspectus rebuild <pin|h_id|node|mux> [--dry-run]`
  brings a gone mux (resuming its last session), session, worktree
  (through worktrunk), or repo (`git clone` into an absent path) back as
  a plan that stops at the first failure. Launch argv comes from the
  pin, the recorded launch, `[launch.<harness>]` profiles, `atelier
  exec`, then the harness default.
- **Pinned and History views** (`4`, `5`), ghost muxes in the Mux view,
  a `.` row menu with Pin / Unpin (`p`), a `◉` pin marker, a `pinned
  only` filter, and `N` for mux launch with a worktree toggle.

### Changed

- **Breaking: pins moved and the pin verbs changed.** Pins live in
  `$XDG_DATA_HOME/conspectus/pins.toml`, not as `[[pins.entries]]` in
  config files. `pin create`, `launch`, `attach`, `bind`, `rebind`, and
  `adopt` are retired and print their replacement (`pin add`,
  `conspectus rebuild`, `pin retarget`). Legacy entries still load, read
  only, with a diagnostic until `conspectus pin migrate` converts them
  and removes the old tables. The Pins modal and the `B`, `b`, `A`, and
  `L` keys are gone.
- **Breaking: the hidden `union` and `forks` TUI views are removed.**
  Neither was in the view switcher, and their groupings did nothing.
  `conspectus tui --view union|forks` now fails and lists the valid
  views, and `[tui] default_view = "union"` or `"forks"`, or a
  `[tui.views.union]` or `[tui.views.forks]` table, is reported as a
  config diagnostic and ignored. `conspectus table union|forks` is
  unchanged.
- **`[tui.theme]` has two muted keys instead of five, and reaches the
  forms.** `muted` (a modifier, `dim`) and `inert` (a color, dark gray)
  replace `placeholder`, `divider`, `mux_unmuxed`, `secondary_text`, and
  `ghost`, which still load as aliases; set several of them differently
  and they merge by a fixed precedence, with a warning for each key that
  lost. Setting only `placeholder` now also restyles rules and the
  unlinked `∘`, and setting only `secondary_text` also recolors gone
  rows. New keys: `accent`, `link`, `info`, `surface`, `field`,
  `field_active`, `field_cursor`, and `command_badge_text`; a style may
  name a background (`white on dark_gray`). The mux launch, new mux, and
  pin edit forms, the previews, and the transcript viewer now follow the
  theme; the default look is unchanged. `docs/operations.md` lists every
  key, its default, and the older names.
- **Wrapping follows one rule across the TUI.** A word that ends exactly
  at a row's edge stays on that row (help, the viewers, modals, and
  previews used to move it down a row); key hints wrap between hints, so
  a key is never split from its verb and no row starts with `·`; the
  transcript viewer keeps the blank lines between Markdown paragraphs,
  lists, and code blocks; and a cut form value or quick select snippet
  ends in `…` instead of stopping at the edge.
- **Search snippets favor the text after the match.** A long message's
  snippet, in the TUI and in `conspectus search --json` (`snippet.text`),
  is at most 160 terminal cells with its `…` marks (it was 160 characters
  plus the marks) and spends two thirds of its room after the match,
  cutting on word edges, instead of centering the match.
- **Ages read one way across the TUI.** The Search view's match, pane,
  and pane-line rows color their age by how recent it is, as every other
  row does (they were all gray). The search status says `scanned 0s ago`
  rather than `scanned just now`, and a scan a day old reads `1d ago`
  rather than `25h ago`. The History view's Clean up list counts days
  (`last seen 21d ago`) like the History rows, not weeks. The CLI's
  tables are unchanged and still go on to weeks.
- **`[tui.theme] badge_width` sizes every chip.** The content search's
  chip column and a mux's ` no agent ` placeholder now follow it, so with
  a narrower or wider chip the labels after them stay in one column. The
  default look is unchanged.
- **Modals share one frame and five sizes.** Every menu, prompt, form,
  list, and reader draws the same border and title (plain, one space each
  side: the Messages and value titles are no longer bold) and takes one of
  five sizes. Menus are 64 columns (the row menu was 72, the mux menu 62,
  the worktree menu 56), forms 80 (the pin edit form was 84, the others
  76), the picker and the value modal the help's size, and the help two
  rows taller. Every modal now leaves two columns and a row clear around
  it, so a menu or form on a short terminal no longer covers the header
  and status line (it scrolls), and a menu on a narrow one no longer
  touches the screen's edges. A short worktree menu is only as tall as
  its entries. Below 44 columns the forms fit their key hints to the
  frame instead of cutting them, and the empty Clean up wraps its text.
  `[tui.theme] surface` now also paints the modals' background.
- **Every list shows its cursor the same way.** The row the cursor is on
  is marked `►` in the two cells before it and drawn with
  `[tui.theme] selection_active` (reversed and bold by default), in the
  menus, Controls, the picker, the checklists and Clean up, quick select,
  content search and its options, the forms, Messages, and the right pane.
  The menus were bold only and Controls, the picker, the search lists, and
  the forms showed `>`. While the tree has focus the right pane keeps its
  cursor row marked in `selection_inactive` (bold) instead of showing no
  cursor, and with focus it highlights the whole row rather than one
  value. The menus now wrap from their last entry to their first and back,
  like the other short lists. Messages' entries move two cells right for
  the mark, the search lists' cursor row keeps its dimmed context, a
  form's focused row highlights its label around the field's own fill, a
  conflict row in the right pane's Other zone shows its `▲` two cells
  further in, and the checklists cut a long label with `…`. The left
  pane's look is unchanged.
- **A letter in a dialog means the letter, not its chords.** Inside
  menus, prompts, forms, Controls, search, Messages, the help page, the
  value viewer, and the transcript and pane viewers, a letter key fires
  plain or with Shift only. `Ctrl-G` and `Alt-G` no longer jump to the
  top (and `Alt-G` to the bottom) of the help page, the value viewer, and
  Messages; `Ctrl-G` no longer jumps to the top of the viewers; `Ctrl-D`
  no longer discards a branch in the close-down choice, and `Ctrl-Y` no
  longer confirms a worktree removal, merge, or prune or copies in the
  value viewer. Every list now also moves with `Ctrl-N` and `Ctrl-P`: the
  row, mux, and worktree menus, Clean up, Controls and its checklists,
  Messages, and the fields of the new tmux session, launch, and pin edit
  forms, as search and the picker already did.
- **One way out of every dialog.** `Esc` steps back one level and
  closes a dialog at its first; `Ctrl-C` closes the whole dialog from any
  level, and so does `q` wherever nothing is typed. So `q` now closes the
  row, mux, and worktree menus, Clean up, Controls (and its checklists,
  but not its max-age and contents fields, where it types), the worktree
  confirmations and close-down choice (which it used to back out of), and
  the content search's options; `Ctrl-C` in the worktree menu's branch
  prompt, confirmations, and close-down choice closes the menu instead of
  returning to its list; and in the transcript and pane viewers `q` and
  `Ctrl-C` leave from the help page and `Ctrl-C` from the find prompt
  (they used to close only the help or the prompt). Cancelling says
  nothing: the rename prompt, the worktree menu, the new tmux session
  form, the mux menu, the launch form, and the pin edit form no longer
  report `…: cancelled`, and leaving the transcript viewer no longer
  reports `viewer closed`. Closing any dialog with a key clears the
  status line, so a hint it showed while open (`controls: …`, `viewing
  …`) no longer stays behind.
- **Dialogs draw as a stack.** Every open dialog is drawn, bottom to
  top, so one opened over another shows the lower one around its frame
  (only the top one used to be drawn), and a toast now shows over the
  full-screen transcript and pane viewers, which used to hide it.
- **Feedback says how much it matters.** Every status-line message,
  toast, and Messages entry has a severity (info, success, warning,
  error) and draws in its `[tui.theme]` color. A warning toast's border
  is yellow and an error toast's red (every toast used to be green, so
  `copy failed` looked like `copied`); warning toasts stay 3 seconds and
  error toasts 5 (all used to leave after 1.5). The status line draws
  hints and confirmations green and failures red (it painted everything
  yellow; refusals stay yellow). On a screen narrower than 50 columns a
  long toast wraps inside the screen instead of losing its end, and a
  word longer than a toast's row, such as a long path, now breaks at the
  row's edge rather than at a hyphen. A clipboard write that fails on
  `Enter` or `i` says `copy failed: …` instead of `copied: …`.
  `[tui.theme] toast_text` (white) sets a toast's text color. Successful
  toasts and the default screens look as before.
- **Dialog footers read one way.** Every footer pairs keys with a short
  verb: `to scroll`, `to close`, `to copy`, `to confirm`, and `to cancel`
  lose their "to" (the help page, the value viewer, the worktree
  confirmations), the close-down choice reads `m merge & close`, search
  reads `Tab search current/all`, `S-Tab list sessions/messages`,
  `Ctrl-R recall search`, `Ctrl-O set options`, and `Ctrl-S open Search
  view`, and the forms read `←/→/Space change`. The value viewer's footer
  lists its keys in the help page's order and names `PgDn`/`PgUp`.
- **Every text field edits the same way.** The transcript and scrollback
  find prompts take the keys every other field takes (`Ctrl-A`/`Ctrl-E`,
  `Ctrl-B`/`Ctrl-F`, `Ctrl-W`, `Ctrl-K`, `Ctrl-Y`), and `Ctrl-U` there
  clears the whole query instead of only what precedes the cursor. Every
  field also moves by word on `Alt-←`/`Alt-→` and deletes the next word on
  `Alt-D`. The prompts' and find prompts' cursor cell takes the
  `field_cursor` style of `[tui.theme]` (black on white), as the forms'
  did, instead of reverse video.
- **Prompts explain a refusal inside themselves and show their keys.**
  The rename, worktree branch, and controls max-age and contents prompts
  show `Enter confirm · Esc cancel` on their bottom border (the branch,
  max-age, and contents prompts showed no keys; the renames showed them
  on the status line, which now stays as it was). Input a prompt refuses
  is explained on an error line inside it, which grows the prompt by one
  row until the next edit: an invalid max age and an empty branch name
  used to post a toast, and renaming a pin or a tmux session to nothing
  closed the prompt with a toast (`pin: nothing entered`, `mux rename:
  name cannot be empty`) where it now stays open for a fix.
- **Path fields show one hint, and give the path the rest of the row.**
  The new tmux session and launch forms kept 32 cells after every path
  for a `Tab: …` hint, so a 60-column form showed four characters of the
  path; the hint now shows, and takes its cells (at most 30, and at most
  half the row), only while the field is focused and `Tab` has something
  to add, and the pin edit form's path rows show it too. The hint names
  exactly what `Tab` adds, and a long one is shortened in the middle.
- **Forms submit from any field and keep their keys in view.** `Enter`
  in the new tmux session form creates the session from either field (it
  used to move from the name to the cwd first; the footer reads `Enter
  create`), and opening it with `n` no longer puts its keys on the status
  line, since the form shows them. An empty name or cwd is explained on
  the form's error line, just above its keys, until the next edit (it was
  a toast), and the launch form's error line no longer has a blank line
  between it and its keys. On a short terminal the new tmux session and
  launch forms scroll only their fields and keep their keys in view, as
  the pin edit form did; their keys used to scroll away. The launch form's
  fields read `name`, `socket`, and `argv` (they read `mux name`, `mux
  socket`, and `launch argv`), and its `pin` choices are drawn as a choice
  of one, `(•) don't pin ( ) launch, then pin ( ) pin, don't launch`,
  instead of as checkboxes.
  The launch form's worktree repo completes and is marked like the cwd.
  A path that is missing or not a directory is marked `▲` (it was `!`).
  Typing a name that only appears inside a known directory (`src` for
  `~/work/src/app`) no longer completes to it; type the start of the
  path.
- **The launch form moves on one ring.** Its socket checkbox, which sat
  at the end of the name row and only `Tab` reached, is a `server` row
  of its own under the name (`[ ] own socket`), so `↓` and `Tab` visit
  the same fields and the name gets the whole row; the `socket` field
  still appears under it while it is on. A launch the form refuses moves
  the cursor to the field it is about, as the new tmux session form
  does.
- **One way to split an argv.** The launch form's `argv`, the pin edit
  form's `argv` (its checkboxes and its save), `pin edit --set argv=…`,
  and the previews of a tmux pane's start command split text the same
  way, as `docs/operations.md` describes. A backslash inside double
  quotes before an ordinary character now stays (`"a\b"` is `a\b`; the
  previews and `pin edit` read `ab`), a last lone backslash stands for
  itself, and the pin edit form and `pin edit --set` refuse an argv that
  ends inside a quote or on a lone backslash (they closed the quote or
  dropped the backslash). The forms write an argv back quoted as the
  previews quote it, so `@`, `%`, and `,` stay bare.

### Fixed

- **A file no longer passes for a working directory.** The TUI's path
  fields offered files as completions and marked an existing file `√`, a
  path `mux new`, `mux launch`, and a rebuild then refused. They now
  offer only directories and mark a file `▲`.
- **Drawing a form reads no files.** A path field listed a directory and
  checked its path on every frame drawn; it now looks only when its text
  changes. The launch and pin edit forms looked for an Atelier workspace
  on every frame to show the default command; they now decide it when
  the form opens and when the harness or the cwd changes.

- **Text fields keep their cursor in view.** The quick select, content
  search, and picker queries show a cursor and scroll to keep it on
  screen (a long query used to be cut off at the frame with no cursor).
  CJK and emoji text puts the cursor on the character it is on and stays
  inside its field in the prompts and forms, so it no longer runs past
  the frame or pushes a path's mark or the pin form's last column aside.
- **The `?` help names the right key and keeps its headings whole.** The
  content search row said `Ctrl-L` switches between listing sessions and
  listing messages; inside the search that is `S-Tab`. A heading too long
  for a narrow help page wraps instead of being cut off at the frame.
- **Wide text lines up in the TUI.** CJK, emoji, and accented text is
  cut and padded by the cells it takes, not its characters, wherever a
  row, chip, form, or find chip cuts text: a 30-character CJK title is
  cut at the right pane's 48-cell budget instead of wrapping, an emoji
  written with a variation selector (`⚠️`) no longer pushes its row a
  cell past its column, and no cut separates a letter from its accent.
  A value just over that budget now offers `o` for the full value too.
- **The rows the TUI counts are the rows it draws.** The right pane, the
  preview, the left pane's notices, and the worktree menu wrap their text
  before measuring it, so a scrollbar matches its content, the right
  pane's last row is no longer hidden behind a spare blank row, and the
  worktree menu no longer cuts off its last lines at narrow widths. A
  numbered tool-output line in the transcript viewer that wraps hangs
  under its text instead of running past the pane's edge.

- **Paging and scrolling in the TUI stay on the content.** `PgDn`/`PgUp`
  move by the rows on screen in the tree, help, the full-value viewer,
  the Messages record, and the viewers' help pages (the tree stepped by
  the terminal height and the dialogs by 8 lines). `G`, or paging past
  the end, of a full value or a Messages record shows its last lines
  instead of a blank body, and the next key up moves at once. A tree
  that shrinks no longer leaves blank rows below it, the `.` row menu
  scrolls to its cursor and keeps its key hints, and the row, mux, and
  worktree menus no longer crash a very short terminal.
- **The right pane stays on its content too.** Scrolling the Preview
  (`J`/`K`) past its end shows its last lines instead of blank rows, and
  the first key back moves it at once; the explorer fills its zone after
  its rows shrink (hiding edge metadata at the end of a long Related list)
  instead of leaving the last row over blank rows; and a field that wraps
  to more rows than the zone shows its label first.
- **`Enter` and `o` work on every field the right pane shows.** With
  Expanded Node Detail on (`F`), the fields past the first five did
  nothing: `Enter` did not copy them, and `o` said the row had no
  truncated value even beside a `(truncated · o)` hint. Both now act on
  the field under the cursor, expanded or not.
- **A pull request's state reads one way everywhere.** A closed or merged
  PR still marked draft drew a draft-yellow `⇄` beside its red or
  magenta state glyph. The state now beats the draft flag in every cell,
  view, and group: only an open draft reads as a draft, and a PR with no
  state reads `?` (`unknown` in Pinned and History), draft or not.
- **Group headers line up.** The label, path, and count columns are
  measured over the whole tree, so unfolding a group no longer moves the
  others' columns (a long name inside a folded group widens them
  instead); the counts line up when a pin marker is shown, and
  `Ungrouped` and a PR group with no local repo start their name in the
  column of the headers beside them.
- **Chips stay chips on the selected row.** A program chip no longer
  turns into the selection highlight; like a harness chip, it keeps its
  look.
- **A paste can't submit a TUI prompt or form.** The TUI turns on
  bracketed paste, so a multi-line paste is typed into the field as one
  line (line breaks become a space) instead of pressing Enter at the end
  of its first line. A paste outside a text field (the tree, a menu, a
  form's option row) is ignored rather than read as keys.
- **Alt-B and Alt-F move by word in every TUI text field.** They did
  nothing; the transcript and scrollback find prompts also gain word
  motion on Alt-B/Alt-F and Ctrl-←/Ctrl-→.
- **`~` in a TUI path field reaches the command expanded.** The new tmux
  session form, the launch form's cwd and worktree repo, and a pin's
  path fields expand a leading `~` when they commit, so `mux new`, `mux
  launch`, and rebuilds get the directory instead of rejecting a literal
  `~/…` path the form had shown as valid.
- **Refused input says why.** An invalid max age in the controls, an
  empty branch name for a new worktree, and an empty name or cwd in the
  new tmux session form say what is wrong (each on its error line), where
  they used to do nothing; the field stays open for a fix.
- **The view tabs fit their pane.** At 100 to 104 columns, and below 53
  in the stacked layout, the pane's border cut the last view's name in
  half (`sea`, `his`), the active one included. Names that don't fit now
  give way from the end of the strip, the active view's name always
  shows, and the strip ends with `…`.
- **The header's session count matches the view.** Pinned, History, and
  Search before a search read `0/N sessions`, and PRs read `4/N` with no
  filter, as if sessions were filtered out. The count reads `N sessions`
  unless a filter or a search narrows what the view lists; Search counts
  the sessions its search found.
- **Menus close in `tui --fixture` and `dev scenario tui`.** A menu or
  form opened there (`.`, `N`, or a menu entry) ignored every key, Esc
  and Ctrl-C included, so the TUI could not be closed. It now moves and
  closes as usual, and an entry that would write or launch says it isn't
  available in this mode.
- **Harness hooks work while `conspectus serve` runs.** The daemon
  applied each hook observation to its live graph only, and the next
  refresh, a few seconds later, dropped it. It now records the
  observation in `hooks-latest.json` first, so hook attribution lasts
  across refreshes and daemon restarts.

## [0.1.0] - not yet released

The first public release, 0.1.0. Conspectus builds one evidence-backed
graph of the AI-agent work on your machine and gives you a TUI,
scriptable tables, and exports on top of it. See the
[README](README.md) for the overview.

### Added

- **Discovery** of agent sessions from Claude Code, Codex, opencode,
  and aider (titles, last-message previews, compaction/resume/fork
  lineage); tmux sessions, plus zellij for discovery and attach; git
  repos, checkouts, linked worktrees, and branches; Atelier,
  agent-deck, and generic multi-repo workspaces; and GitHub pull
  requests through `gh`: open PRs and the last 30 days of history for
  every GitHub remote of every discovered repo, including a fork's
  upstream and remotes on any GitHub Enterprise host `gh` is signed in
  to, linked to a local branch when the PR's head is one of the repo's
  remotes. Forge failures surface as diagnostics.
- **Evidence-preserving resolution.** Every relationship is a
  `GraphLink` candidate with provenance, confidence, and freshness; the
  resolver picks preferred links and keeps the rest, so ambiguity and
  conflicts stay visible (`graph --explain` shows score breakdowns).
  Candidates that agree with the winner are kept as corroborating
  evidence rather than reported as conflicts.
- **Session-to-pane attribution** from layered evidence: opt-in harness
  hooks (`conspectus hook init|status|remove` for Claude Code and Codex,
  plus an opencode plugin), open transcript file descriptors, Codex
  state and log databases, process-tree walks, launch commands, and
  working directories.
- **Interactive TUI** (`conspectus` or `conspectus tui`): Sessions,
  Mux, and PRs views (`1`/`2`/`3`), with graph/workspace/repo/checkout/flat
  grouping for sessions and checkout/repo/state grouping for PRs, where
  a marker shows whether each PR has a local branch, a filter and
  ranking (also settable from config) can include, exclude, or float
  those that do, a state filter keeps only open, draft, merged, or closed
  PRs under any grouping, PRs checked out nowhere sit under their repo, and each
  row shows its number, state, age, and title in aligned columns under a
  group that names the `owner/repo`;
  filters and search; a relationship explorer with one row per
  neighbor, drill-down, and breadcrumbs; a native transcript viewer for
  Claude Code, Codex, and opencode; menu-first controls; and
  `[tui.theme]` theming.
- **Node-specific previews** in the TUI: a session's transcript, a tmux
  session's live pane (with smart, plain, or no wrapping), a checkout's
  `git status` and files, a repo's recent commits, a branch against its
  upstream, a PR's checks and reviews, and a process tree. Moving onto a
  field previews its value, and moving onto a relationship shows the
  link's evidence above the neighbor's preview.
- **Refresh controls**: the TUI refreshes in the background, `r`
  rescans now, and `Ctrl-R` is a hard refresh that also asks GitHub
  again. Forge data is otherwise fetched in the background, off the
  daemon's writer lock, when a repo appears and then hourly (`[server.intervals]
  forge`); no other refresh waits on it. `[forge] concurrency` caps
  concurrent `gh` calls (default 4).
- **CLI views**: `conspectus table sessions|mux|union|prs|forks` with
  column selection, card layout, paging, and color; `conspectus columns`;
  `conspectus node show`.
- **Graph exports**: `conspectus graph --format json|dot|html`, where
  HTML is a self-contained interactive explorer.
- **Declared links and aliases**: `conspectus declared
  list|create|remove|confirm|ignore|override`, `conspectus rename
  session|mux` (alias plus lockstep tmux rename), and `conspectus alias
  list`, all stored as reviewable TOML.
- **tmux lifecycle**: `conspectus mux new` (bare shell) and `conspectus
  mux launch` (a harness in a fresh tmux session).
- **Worktrees**: `conspectus worktree list` (read-only) and
  `new|rm|merge|close|prune`, delegated to
  [worktrunk](https://github.com/max-sixty/worktrunk). `worktree close`
  winds a stream down: it stops the stream's tmux sessions (graceful, then
  forced), merges or discards the branch, removes the worktree, and
  forgets the history records rooted in it.
- **Continuous mode**: `conspectus serve` keeps the graph warm with
  per-source refresh intervals (harness and mux 5s, git 30s, forge 1h)
  and filesystem watchers, serves it over
  a Unix socket, and saves a zero-copy `graph.bin` for warm restarts;
  `conspectus status` and `conspectus refresh` inspect and drive it.
  Every command still works without the daemon.
- **`csp`**, the official short name, installed alongside `conspectus`.
  It is the same program; help and suggested commands use the name you
  ran.
- **Library API** at `conspectus::api` (not yet a stable contract).
