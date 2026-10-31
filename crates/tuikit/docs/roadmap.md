# Roadmap

Controls and capabilities that other terminal UI frameworks and well-regarded
terminal programs ship and this framework does not, ranked by how soon they
are likely to be worth adding, with what is deliberately left out. Nothing
here is scheduled: an item is built when a program on the framework needs
it. The controls that have landed keep a seam open for the items that touch
them, described under "Seams left open" on their pages, so adding one later
does not break their API.

The list comes from a survey of more than a hundred projects: Rust crates on
ratatui and beside it, frameworks in Go, JavaScript and TypeScript, C and
C++, Haskell, Zig, and .NET, Python's Textual, Rich, prompt_toolkit, and
urwid, web design-system component lists, and fifteen terminal programs.
Commonness counts below are out of six Python and Go frameworks unless they
say "programs" (out of fifteen, and floors: a control counts only where a
project's own pages mention it).

| Tier | Meaning |
| --- | --- |
| Tier 1 | Cheap now, or needed soon; each is marked done or open below |
| Tier 2 | Add when a program needs it; the seam is kept open |
| Tier 3 | Only when a feature calls for it |
| Skip | A poor fit for a terminal, or for the framework |

A new third-party crate for any item needs a decision record first (see
[decisions/README.md](decisions/README.md)): the survey found the young
design kits (`tuika`, `tui-lipan`, `ratatui-themekit`) churning fast, so they
are better read for ideas than depended on.

## Tier 1

| Item | Status | Where it stands |
| --- | --- | --- |
| Select all and none in the checklist, with a count | done | `a` and `n` check every item that can change, and the bottom border counts what is checked ([checklist.md](controls/checklist.md)) |
| Style-aware snapshot goldens | done | `testkit::golden` keeps every color and modifier, so a change of color alone fails; goldens are accepted through `TUIKIT_ACCEPT_GOLDENS` ([testkit.md](controls/testkit.md)). Snapshot crates on ratatui's test backend drop styles, which is why the kit has its own |
| Bracketed paste | done | a paste is typed as one line (`text_edit::single_line_paste`, `LineEdit::paste`) and only into an overlay whose text field has the keys (`Overlay::takes_text`); the host turns the terminal mode on ([text_edit.md](controls/text_edit.md)) |
| Action availability (show, dim, hide) | done for show and dim | key rows carry `available` (`keys::with_availability`) and keymap bindings a predicate (`available_when`), so footers, help pages, and the status bar dim what cannot act now from one source; hiding is a filter over the same copy ([keys.md](controls/keys.md)) |
| A spinner | done | `chip::spinner_frame` and `StatusChip::busy` draw the busy frame from how long the work has run, with the frames vetted in the glyph table ([chip.md](controls/chip.md)) |
| A command palette | open | a `Palette` over rows built from a key table's shown rows or a keymap's bindings, filtered with `every_word`, committing the row's action; the palette and the keymap are ready, and the seam is described in [palette.md](controls/palette.md). Textual's command providers show a scoring shape worth reading |
| Theme presets, `NO_COLOR`, and color-depth downgrade | open | a preset is a configuration table applied before the user's own ([theme.md](controls/theme.md)); nothing detects `NO_COLOR` or the terminal's color depth yet. The pointer mark (house rule 1) and the glyph-plus-word status already carry meaning without color. `tui-nuances` and `ratatui-themekit` show both detections |

## Tier 2

| Item | Commonness | The seam |
| --- | --- | --- |
| A multi-line text area | 4/6 | a control of its own beside `LineEdit`, not an option of it ([text_edit.md](controls/text_edit.md)); `ratatui-textarea` is the maintained engine (`tui-textarea` 0.7 does not build against ratatui 0.30) |
| Validated and typed input (numbers, patterns, masks) | 4/6 | a prompt's `on_confirm` and a form field's check already refuse with a reason (house rule 7); a live check called on each edit is the next step ([text_prompt.md](controls/text_prompt.md), [form.md](controls/form.md)) |
| A general completion source | 3/6; programs 4 | `PathField`'s `PathSource` and its ranked candidates, generalized when a second kind of field completes ([path_field.md](controls/path_field.md)) |
| Plain-text tables from the column schema | 4/6 | a table printed outside the terminal UI laid out by the same `ColumnSchema` ([columns.md](controls/columns.md)); Textual's `DataTable` is the broadest sortable, filterable table found |
| A progress bar | 4/6 | `palette::Progress` already words how far a slow provider has got; a `Progress` control when a second long operation appears |
| A resizable split | rare | a share for `SplitPane`'s first pane, or one that widens the focused pane ([frame.md](controls/frame.md)) |
| Light and dark adaptation | programs 5 | a variant table applied before the user's own ([theme.md](controls/theme.md)); terminals report their background through mode 2031 |
| Mouse hit testing | programs 9 | every control's page says what a click would be tested against (a frame's rect, a window's visible rows, a line's kept pieces); a click-region registry when the first mouse work lands. `ratatui-interact` shows a drag model |
| User key remapping | 3/6; programs 9 | keymap bindings have stable ids, and the build already refuses overlapping keys, so a host replaces the keys of the bindings its configuration names before `build` ([keys.md](controls/keys.md)) |
| A density token | design systems | a `density` token on the theme for compact rows |
| A declared focus cycle | the original control set | focus rules live with `PaneFrame`, the overlay contract, and the keymap's pane scopes ([architecture.md](architecture.md#focus)); a declared order of panes for `Tab` would be the framework's part of a host's focus state |

## Tier 3

| Item | When it would be worth it |
| --- | --- |
| A pending-chord popup (which-key) | only once keys of several presses exist |
| A dropdown select | a field with more options than a strip of chips can show |
| A file browser | a browse mode beside `PathField`, reading the same `PathSource` ([path_field.md](controls/path_field.md)) |
| Diff and structured-data viewers | a `Pager` content of their own covers a read-only view ([pager.md](controls/pager.md)) |
| Sparklines and inline graphs | ratatui ships `Sparkline`; only if activity over time becomes a column |
| Hyperlinks (OSC 8) | never as the only way to reach a target |
| A background task list | when a host runs several long tasks the user should see together |
| Synchronized updates and the kitty keyboard protocol | crossterm exposes both; the kitty protocol is what lets `Ctrl-I` and `Tab` differ |

## Skip

A switch (a labelled checkbox serves), sliders, date pickers, steppers, and
color pickers (web conventions with no terminal habit behind them),
pagination (pagers and lists scroll), tooltips (a terminal has no hover; the
generated footers and hints do the job), context menus at the pointer, image
and chart widgets, plugins and an extension model, and animation for now
(`tachyonfx` composes as a pass over any drawn buffer, so deferring it costs
nothing).

## Already in the set

The controls the survey found common that the framework already has: one-line
text entry, checkboxes and one-of-N rows (with distinct shapes, `[x]` and
`(•)`), a filtered picker, a form, an action menu, a dialog, a list, a tree,
a pager with find, a log reader, Markdown and highlighted code (the
`richtext` feature), a breadcrumb, collapsible sections, tabs, an empty
state, toasts with severities, a generated help page, a key-hint footer and
status bar, an OSC 52 clipboard sink, and non-color status carriers. Three
things the survey found rare are here too: key tables that generate the
footers, hints, and help from the same rows that resolve the keys; controls
that report intent outcomes instead of acting; and a conformance suite that
holds every modal a host registers to one set of conventions.
