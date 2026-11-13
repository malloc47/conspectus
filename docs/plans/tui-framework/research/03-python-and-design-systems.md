# Research 03: Python TUI frameworks, design systems, and TUI conventions

Evidence base for the gap analysis (`gap-analysis.md`). Research date: 2026-10-27.
Scope: Part A, Python frameworks (Textual, Rich, prompt_toolkit, urwid,
asciimatics, pytermgui and adjuncts). Part B, design-system taxonomies,
accessibility guidance, keyboard conventions, terminal-capability concerns, and a
survey of 15 well-regarded TUI apps.

## 0. Method and limits

- Only `WebFetch` was usable (the `WebSearch` tool was rejected by the model
  backend), so every fact comes from a fetched page. Nothing was cloned,
  downloaded, or executed.
- `WebFetch` answers through a summarising model. Treat exact numbers (stars,
  dates) as "as reported by the fetched page on 2026-10-27". Where the page did
  not state something, it is marked **unverified**.
- Claims carry a source key in brackets, for example `[T-gallery]`. The key to
  URL index is in section 9.
- "Apps" counts in section 2 are a **floor**: a control is counted only if a
  fetched page mentions it for that app. An absent count does not prove absence.
- One fetch (the Textual "widgets" guide) was refused by a safety filter and was
  not retried; nothing in this file depends on it.

## 1. Currency and maintenance snapshot (as of 2026-10-27)

| Project | Latest (per page) | Date | Maintenance read | Source |
| --- | --- | --- | --- | --- |
| Textual (Python) | 8.2.8 | 2026-06-30 | Releases roughly every 2-4 weeks from 2026-03 to 2026-06 (8.1.1 03-10, 8.2.0 03-27, 8.2.3 04-05, 8.2.5 04-30, 8.2.7 05-19, 8.2.8 06-30). No newer release visible ~4 months later (possible slowdown; unverified). 37.5k stars, MIT. Company Textualize wound down in 2025; Will McGugan says he keeps maintaining Textual and Rich as open source. Repo has an `AI_POLICY.md` requiring maintainer pre-approval of AI-written PRs. | [T-pypi] [T-rel] [T-chg] [T-repo] [T-future] [T-ai] |
| Rich (Python) | 15.0.0 | 2026-04-12 | Active, same maintainer. Docs mention new env vars `TTY_COMPATIBLE` / `TTY_INTERACTIVE` and nested `Live` since 14.0.0. | [R-pypi] [R-console] [R-live] |
| prompt_toolkit | 3.0.53 | 2026-07-26 | Active; requires Python >=3.10. 10.6k stars, BSD-3-Clause, 621 open issues. | [PT-pypi] [PT-repo] |
| urwid | 4.2.5 | 2026-10-05 | Active; README lists CPython 3.11-3.15, free-threaded 3.13+, PyPy, experimental GraalPy (PyPI metadata says `>=3.9`, a mismatch). LGPL-2.1, ~3.0k stars. | [UR-pypi] [UR-repo] |
| asciimatics | 1.15.0 | 2023-10-25 | **Stale**: no PyPI release in 3 years. 4.3k stars, Apache-2.0, 20 open issues; commit recency not visible. | [AS-pypi] [AS-repo] |
| pytermgui | final release | archived 2026-09-10 | **Archived, read-only**; README says "no longer in development". 2.7k stars, MIT. | [PG-repo] |
| Trogon | 0.6.0 | year unverified | Auto-generates a Textual TUI from a Click CLI; self-described "early stage"; 2.8k stars. | [T-trogon] [T-trogon-rel] |
| textual-web | beta | n/a | README says beta; 1.5k stars; recency unverified. | [T-web] |
| textual-serve | 1.1.3 | 2025-11-01 | Serves a Textual app in a browser over websocket. | [T-serve] |
| textual-dev | 1.8.0 | 2025-10-11 | CLI devtools (`textual console`, `run --dev`, `colors`, `keys`, `borders`, `easing`). | [T-devpkg] [T-dev] |
| pytest-textual-snapshot | 1.1.0 | 2025-01-23 | `snap_compare` fixture, SVG baselines. | [T-snap] |
| Terma (Go, Darren Burns) | no releases | n/a | Go TUI library; docs list 34 widget pages. Public repo shows 4 stars, 647 commits, MIT. Posting 3 is built on it. | [TM-repo] [TM-index] [TM-widgets] |
| Posting | 3 (beta) | n/a | **Moved from Textual (Posting 2) to Go + Terma (Posting 3).** 12.5k stars, Apache-2.0. Notable signal about Textual's flagship app. | [PO-readme] [PO-repo] |
| huh (Go, Charm) | n/a | n/a | Form library with an accessible mode; 7.2k stars, MIT. | [HUH] |

Apps surveyed in section 2 (latest tagged release per the releases pages):
lazygit v0.66.2 (2026-10-16), k9s v0.51.0 (2026-06-06), yazi v26.9.1
(2026-09-01), btop v1.4.8 (2026-10-25), zellij v0.45.1 (2026-08-28), atuin
18.24.2 (2026-10-22), helix 25.07.1 (2025-07-18; infrequent tags), gitui v0.28.1
(24 Mar, year not shown). Sources: [LG-rel] [K9-rel] [Y-rel] [BT-rel] [ZJ-rel]
[AT-rel] [HX-rel] [GU-rel].

## 2. (a) Control catalogs

### 2.1 Textual (the widest control set)

Textual's gallery lists 35 widgets [T-gallery]; the reference index lists 37 and
adds `ListItem` and `Toast` [T-widgets]. `HelpPanel`, `KeyPanel` and `Tooltip`
exist in source [T-help] [T-tip] but I could not find standalone doc pages
(unverified status in the public docs).

| Control | Provider | API shape | Notable behavior | Src |
| --- | --- | --- | --- | --- |
| Button | Textual | Clickable button | "Several meaning-based colour styles" (variant names not shown on the fetched page) | [T-gallery] |
| Checkbox / Switch | Textual | `Checkbox`, `Switch` | Switch is a distinct on/off toggle | [T-gallery] |
| RadioButton / RadioSet | Textual | `RadioSet(...)` one-of-N | Group semantics built in | [T-gallery] |
| Input | Textual | `validators=[...]` (page names `Number` and `Function`; others "built in", list not shown), `validate_on`, `suggester`, `type` (text/integer/number), `password`, `restrict` (regex over whole value), `max_length`, `valid_empty`, `select_on_focus`, `placeholder` | Toggles `-valid` / `-invalid` CSS classes; right-arrow accepts suggestion; readline-like keys (ctrl+a/e/k/u/w, ctrl+f deletes word right) | [T-inputw] |
| MaskedInput | Textual | Template string, e.g. `"9999-9999-9999-9999;0"` | Template chars A/a N/n X/x 9/0 D/d H/h B/b `#`; `>`/`<` case; separators auto-inserted; template is an implicit validator | [T-masked] |
| TextArea | Textual | `language`, `theme`, `show_line_numbers`, `soft_wrap`, `read_only`, `tab_behavior`, `indent_type/width`, `compact`, `placeholder`, `suggestion`, `TextArea.code_editor()` | Tree-sitter highlighting (extra `textual[syntax]`), themes css/dracula/github_light/monokai/vscode_dark, undo/redo with checkpoints (max 50), F6/F7 select line/all, auto-closing-bracket pattern | [T-textarea] |
| Select | Textual | `Select(options, allow_blank, prompt, type_to_search, compact)` | Overlay list; `Select.NULL` for blank; `Changed` message | [T-select] |
| SelectionList | Textual | `Selection(prompt, value, initial_state)`; `selected`, `select_all/toggle_all` | Multi-select over `OptionList`; Space toggles; one bulk message per bulk op | [T-sellist] |
| OptionList | Textual | `Option(prompt, id, disabled)`, `None` as separator | Prompts can be Rich renderables (multi-line); disabled options skipped by cursor; `highlighted`, `compact` | [T-optlist] |
| ListView / ListItem | Textual | list of widgets | Items can be arbitrary widgets | [T-widgets] |
| DataTable | Textual | `add_column(s)`, `add_row`, `cursor_type` cell/row/column/none, `fixed_rows/columns`, `sort(key=, reverse=)`, `zebra_stripes`, row labels | Stable row/column keys survive sort/delete; Rich renderables in cells; Highlighted/Selected/HeaderSelected messages | [T-datatable] |
| Tree / DirectoryTree | Textual | `tree.root.add()/add_leaf()`, `expand_all/collapse_all`, `guide_depth`, `show_root`, `auto_expand` | Hidden bindings for parent/sibling jumps and shift+space expand-all; node messages Expanded/Collapsed/Highlighted/Selected | [T-tree] |
| Tabs / TabbedContent / ContentSwitcher | Textual | Tab row plus panel switcher | Mouse and keyboard; ContentSwitcher shows one child at a time | [T-gallery] |
| Collapsible | Textual | `title`, `collapsed`, `collapsed_symbol` (default ▶), `expanded_symbol` (▼) | Enter toggles; `Toggled/Collapsed/Expanded` messages; nests; no accordion group documented | [T-collapsible] |
| Header / Footer | Textual | `Header()`, `Footer(show_command_palette, compact)` | Footer auto-lists bindings, clickable (per docs), `combine_groups`, shows `^p palette` | [T-footer] [T-gallery] |
| HelpPanel / KeyPanel | Textual | `HELP` markdown string on any widget | Side panel (33% width, 30-60 cols) showing the focused widget's `HELP` (first non-empty up the ancestor chain) plus a key panel | [T-help] |
| Tooltip | Textual | `widget.tooltip = ...`, `Binding(tooltip=...)` | `App.TOOLTIP_DELAY` default 0.5 s; own layer; max-width 40 | [T-tip] [T-appapi] [T-widgetapi] |
| Toast / `notify` | Textual | `notify(message, title=, severity=, timeout=, markup=)` | Severity classes information/warning/error; click or timer dismisses; `NOTIFICATION_TIMEOUT` default 5 s; bottom-right rack | [T-toast] [T-appapi] [T-widgetapi] |
| Command palette | Textual | `COMMANDS`, `get_system_commands()`, `Provider` (`startup/search/discover/shutdown`), `Hit(score, display, callback, help)` | Ctrl+P; fuzzy `Matcher` with highlight; `discover()` fills the empty-query list; per-`Screen` `COMMANDS`; default commands Keys, Quit, Screenshot, Theme | [T-palette] |
| ModalScreen / screen stack | Textual | `push_screen(screen, callback)`, `dismiss(value)`, `push_screen_wait`, `MODES`, `SCREENS` | Modal blocks app-level bindings and dims background; typed result (`ModalScreen[bool]`); modes are independent stacks; Suspend/Resume events | [T-screens] |
| ProgressBar, Sparkline, Digits, Pretty, Rule, Link, Label, Static, Placeholder | Textual | simple widgets | `Placeholder` is design-time filler; `Digits` is big-number rendering; `Rule` is `<hr>` | [T-gallery] |
| LoadingIndicator, `Widget.loading` | Textual | `widget.loading = True` swaps the widget for an indicator | `get_loading_widget()` customises it | [T-gallery] [T-widgetapi] |
| Log / RichLog | Textual | append-only scrolling panes | `Widget.anchor()` keeps a scroller pinned to the bottom until the user scrolls away | [T-gallery] [T-widgetapi] |
| Markdown / MarkdownViewer | Textual | `Markdown` renders; `MarkdownViewer` adds a table of contents and browser-style navigation | | [T-gallery] |
| Text selection | Textual | `ALLOW_SELECT`, `get_selection()`, auto-scroll while dragging | 8.2.0: selection can span containers and auto-scrolls near edges | [T-widgetapi] [T-chg] |
| Maximize | Textual | `ALLOW_MAXIMIZE` (focusable widgets maximizable by default) | Fullscreen a single pane; Footer allowed in maximized view | [T-widgetapi] [T-appapi] |
| Layout | Textual | vertical/horizontal/grid, `dock`, `layers`, `fr` units, `grid-gutter`, spans | Grid recommended over deep nesting | [T-layout] |
| Styling and tokens | Textual | TCSS with `$variables`, pseudo-classes, themes | See section 5 | [T-design] [T-css] |
| Reactives | Textual | `reactive`, `var`, `watch_`, `validate_`, `compute_`, `recompose`, `data_bind` | Compute then validate then watch; `recompose=True` rebuilds children | [T-react] |
| Animation | Textual | `animate(attr, value, duration/speed, easing, delay, on_complete)` | `App.animation_level` defaults from `TEXTUAL_ANIMATIONS` (accepted values unverified) | [T-anim] [T-appapi] |
| Devtools | textual-dev | `textual run --dev`, `textual console`, `textual colors/keys/borders/easing` | Live CSS reload; log/print to a second terminal; `textual serve` for browser | [T-dev] |
| Testing | Textual | `App.run_test()` Pilot (`press/click/hover/pause`), `snap_compare` | SVG screenshot baselines, HTML diff report, `--snapshot-update` | [T-test] [T-snap] |
| Web delivery | textual-serve / textual-web | wrap a command / hosted | Browser rendering of the same app | [T-serve] [T-web] |
| CLI-to-TUI | Trogon | `@tui()` on a Click group | Builds a form from option schema | [T-trogon] |

Widgets announced on the roadmap but not in the catalog: color picker, date
picker, drop-down menus, image support, bar/line/candlestick plots, a form
widget, collapsible markdown sections [T-road]. Accessibility roadmap items
(screen reader, single-colour display, high-contrast theme, colour-blind themes)
are listed as planned [T-road]; the screen-reader issue has been open since
2023-04 with no assignee [T-a11y].

### 2.2 Rich (output renderables, not interactive widgets)

Module list from the reference [R-ref]: align, bar, color, columns, console,
emoji, highlighter, json, layout, live, logging, markdown, markup, measure,
padding, panel, pretty, progress_bar, progress, prompt, protocol, rule, segment,
spinner, status, style, styled, syntax, table, text, theme, traceback, tree.

| Control | Provider | API shape | Notable behavior | Src |
| --- | --- | --- | --- | --- |
| Table | Rich | `Table`, `add_column/add_row`, `box=`, `row_styles`, `show_footer`, `caption`, `Table.grid()` | `ratio`/`min_width`/`max_width` columns, `safe_box=True` ASCII fallback, nested renderables, `end_section` dividers | [R-table] |
| Tree | Rich | `Tree.add()` returns subtree | `guide_style` inherits; bold guide switches to heavy line glyphs | [R-tree] |
| Panel / Padding / Align / Rule | Rich | wrappers | Border, padding, alignment, titled divider | [R-ref] [R-intro] |
| Columns | Rich | `Columns(items, equal=, expand=)` | Responsive multi-column flow (option semantics not documented on the page) | [R-columns] |
| Layout | Rich | `Layout.split_column/split_row`, `size`, `ratio`, `minimum_size`, `visible`, named children | `layout.tree` debug view; pairs with `Live(screen=True)` | [R-layout] |
| Live | Rich | `Live(renderable, refresh_per_second=4, transient, screen, vertical_overflow)` | Redirects stdout/stderr by default; `vertical_overflow` crop/ellipsis/visible; nesting allowed since 14.0.0 | [R-live] |
| Progress | Rich | `Progress(columns...)`, `track()`, `add_task(total=None)` | Columns: Bar, Text, TimeElapsed/Remaining, MofN, FileSize, Download, TransferSpeed, Spinner, Renderable; `total=None` pulses; `transient`; multi-task | [R-progress] |
| Status / Spinner | Rich | `console.status(msg, spinner=)` | Dropped when not a terminal | [R-console] |
| Prompt / IntPrompt / FloatPrompt / Confirm | Rich | `Prompt.ask(..., choices=, default=, case_sensitive=)` | Re-asks until valid | [R-prompt] |
| Syntax / Markdown / JSON / Pretty / Traceback | Rich | renderables | Syntax highlighting, md rendering, pretty-printed data; option details not fetched (unverified) | [R-intro] [R-ref] |
| Console | Rich | `Console(color_system=, force_terminal=, record=)` | Auto-detects size/encoding/colour; `export_svg/html/text`, `pager()`, `capture()`, `screen()` alt-screen | [R-console] |

### 2.3 prompt_toolkit (line editor plus full-screen layouts)

| Control | Provider | API shape | Notable behavior | Src |
| --- | --- | --- | --- | --- |
| Prompt | prompt_toolkit | `PromptSession`, `prompt()` | Emacs default, `vi_mode=True`; `multiline`; `is_password`; `bottom_toolbar`; `rprompt`; `mouse_support`; `show_frame`; syntax highlight via Pygments | [PT-input] |
| Completers | prompt_toolkit | `WordCompleter, PathCompleter, ExecutableCompleter, NestedCompleter, FuzzyCompleter, FuzzyWordCompleter, ThreadedCompleter, ConditionalCompleter, DeduplicateCompleter, DynamicCompleter, merge_completers` | `complete_while_typing`, `complete_in_thread` | [PT-ref] [PT-input] |
| Auto-suggest, history | prompt_toolkit | `AutoSuggestFromHistory`, `FileHistory` | Ghost text; right-arrow / Ctrl-E accepts; Alt-F accepts one word | [PT-input] |
| Validators | prompt_toolkit | `Validator`, `ValidationError`, `from_callable`, `ThreadedValidator` | Live by default; `validate_while_typing=False` defers | [PT-input] |
| Dialogs | prompt_toolkit | `message_dialog, input_dialog, yes_no_dialog, button_dialog, radiolist_dialog, checkboxlist_dialog, progress_dialog` | All built on `Dialog`; style classes `dialog`, `dialog.body`, `shadow`, `button.focused`, `radio-checked`, `progress-bar.used` | [PT-short] [PT-dialogs] |
| Widgets | prompt_toolkit | `TextArea, Label, Button, Frame, Shadow, Box, VerticalLine, HorizontalLine, CheckboxList, RadioList, Checkbox, ProgressBar, Dialog, MenuContainer, MenuItem`, toolbars `ArgToolbar, CompletionsToolbar, FormattedTextToolbar, SearchToolbar, SystemToolbar, ValidationToolbar` | 21 exported names | [PT-widgets] |
| Containers | prompt_toolkit | `HSplit, VSplit, FloatContainer, ConditionalContainer, ScrollablePane, Window`; `BufferControl, FormattedTextControl` | Three levels: containers/controls, widgets, shortcuts; `modal=True` containers stop key-binding inheritance | [PT-full] |
| Progress bars | prompt_toolkit | `ProgressBar` context manager | Formatters Label/Text/Percentage/Bar/Progress/TimeLeft; `patch_stdout()`; cancellation via key binding | [PT-prog] |
| Key bindings | prompt_toolkit | `KeyBindings`, `filter=`, `eager=`, chords, `merge_key_bindings`, `ConditionalKeyBindings`, macros | `ttimeoutlen` resolves Esc vs Alt-prefix; `c-s/c-q` often swallowed by flow control (`stty -ixon`) | [PT-keys] |
| Styling | prompt_toolkit | `class:` names plus `Style.from_dict`; `ColorDepth` 1/4/8/24-bit | `PROMPT_TOOLKIT_COLOR_DEPTH` env; TERM linux/eterm-color forces ANSI; `AdjustBrightnessStyleTransformation` for dark backgrounds | [PT-style] |
| Utilities | prompt_toolkit | `print_formatted_text`, `print_container`, `set_title`, `clear`, `confirm`, `choice` | | [PT-short] |

### 2.4 urwid

| Control | Provider | API shape | Notable behavior | Src |
| --- | --- | --- | --- | --- |
| Text, Edit, IntEdit, IntegerEdit, FloatEdit | urwid | basic widgets | Typed numeric edits with base/separator options | [UR-ref] |
| Button, CheckBox (tri-state), RadioButton | urwid | selectable widgets | GridFlow arranges them | [UR-ref] [UR-widgets] |
| TreeWidget | urwid | nested node display | | [UR-ref] |
| ListBox + list walkers | urwid | `SimpleFocusListWalker`, custom walker V1/V2 APIs | Scrollable list of any flow widget | [UR-widgets] |
| Pile, Columns, GridFlow, Overlay, Frame | urwid | containers | Box/flow/fixed sizing model; Overlay for popups; Frame has header/body/footer focus slots | [UR-widgets] |
| PopUpLauncher / PopUpTarget | urwid | popup helpers | Open/close pop-ups anchored to a widget | [UR-ref] |
| ScrollBar, Scrollable | urwid | scrolling wrappers | Scrollable API `get_scrollpos/set_scrollpos/rows_max` | [UR-widgets] |
| AttrMap, Padding, Filler, Divider, LineBox, SolidFill | urwid | decoration | `AttrMap(focus_map=...)` is the hover/focus state mechanism | [UR-ref] [UR-attr] |
| ANSIText, SelectableIcon | urwid | special text | ANSI escape rendering | [UR-ref] |
| Palette | urwid | named attributes with 16-colour, mono, 88/256-colour and 24-bit values | Explicit **mono fallback**; `set_terminal_properties`; advises degrading gracefully | [UR-attr] |
| Event loops, displays | urwid | asyncio, trio, Tornado, Twisted, ZeroMQ, select; raw, curses, LCD, web (experimental) | Windows support partial | [UR-repo] |

BigText and ProgressBar are mentioned in urwid's docs but I did not see their
reference entries (unverified).

### 2.5 asciimatics, pytermgui, Terma

| Control | Provider | Notes | Src |
| --- | --- | --- | --- |
| Button, CheckBox, RadioButtons, Text, TextBox, Label, Divider, VerticalDivider, ListBox, MultiColumnListBox | asciimatics | Form widgets in `Frame` + `Layout` (proportional columns) | [AS-widgets] |
| DropdownList, DatePicker, TimePicker, FileBrowser | asciimatics | Only surveyed framework with date/time pickers and a file browser | [AS-widgets] |
| PopUpDialog, PopupMenu | asciimatics | Dialog has configurable buttons and is restored after a resize; popup menu closes on Esc or an outside click and is not kept across a resize | [AS-widgets] |
| Effects, Scenes, Themes (monochrome, green, bright, tlj256) | asciimatics | Animation-first model | [AS-widgets] |
| Label, Window, Container, InputField (multiline option), WindowManager | pytermgui | Project archived 2026-09-10 | [PG-repo] |
| autocomplete, breadcrumbs, button, checkbox, commandpalette, dialog, directorytree, empty, filepicker, focustrap, form, image, jsonviewer, jumper, keybindbar, list, markdown, menu, presentedtext, progressbar, select, sparkline, spinner, switcher, table, tabs, text, textarea, textinput, toast, tooltip, tree | Terma (Go) | 34 doc files under `docs/widgets` (two are meta pages). Also docs on animation, drag-and-drop, floating, focus/keyboard, hyperlinks, inline mode, signals, styling, testing, browser-testing | [TM-widgets] [TM-docs] |

## 3. (b) Union taxonomy with commonness

Legend. **F** = frameworks including the control, out of 6:
T Textual, R Rich, P prompt_toolkit, U urwid, A asciimatics, G Terma (Go). Rich is
output-only, so interactive controls rarely count for it.
**Apps** = apps whose fetched docs mention the control, out of 15: LG lazygit,
K9 k9s, Y yazi, BT btop, GU gitui, HX helix, ZJ zellij, AT atuin, TV television,
PO posting, HQ harlequin, FZ fzf, HT htop, BM bottom, LD lazydocker.
**DS** = web design-system evidence: component.gallery example counts
(`n` examples across 60 components; not a count of systems) [DS-gallery],
plus presence in Ant Design [DS-ant], Atlassian [DS-atl], WAI-ARIA APG [DS-apg].
**Band** (the higher of the two metrics wins): Ubiquitous (F>=5 or Apps>=10),
Common (F 3-4 or Apps 5-9), Occasional (F 2 or Apps 2-4), Rare (F<=1 and
Apps<=1).

### 3.1 Input controls

| Control | F (which) | Apps (which) | DS | Band |
| --- | --- | --- | --- | --- |
| Single-line text input | 6 (T R P U A G) | LG K9 HX Y FZ AT TV PO (prompts and filter lines in nearly all) | Text input 72; Atlassian Text field | Ubiquitous |
| Multi-line text area | 4 (T P A G; U unverified) | HX; LG (commit-message keybinding section); PO and HQ implied only | Textarea 52 | Common |
| Validated / typed input (int, float, regex, mask) | 4 (T P U R) | none seen | Date input 18; Ant InputNumber | Common |
| Masked input (template) | 1 (T) | 0 | Ant InputNumber / Mentions | Rare |
| Autocomplete / suggestions | 3 (T P G) | PO HX FZ AT (completion menus, `**`) | Combobox 37; Ant AutoComplete; APG combobox | Common |
| Button | 5 (T P U A G) | BT (clickable key buttons) PO (mouse) | Button 118 | Ubiquitous |
| Checkbox | 5 (T P U A G) | none seen (staging uses Space) | Checkbox 84 | Ubiquitous |
| Radio group | 4 (T P U A) | 0 | Radio 85 | Common |
| Switch / toggle | 1 (T) | 0 (settings are checkbox-like) | Toggle 60; APG switch | Rare |
| Dropdown select | 3 (T A G) | K9 (plugin dropdown input) | Select 82 | Common |
| Multi-select list | 2 (T P; Go huh too) | Y FZ K9 (row marks) LG (range select) | Ant Transfer / Select multiple | Occasional |
| Selectable list / listbox | 5 (T P U A G) | all 15 in some form | List 67; APG listbox | Ubiquitous |
| Slider | 0 | 0 | Slider 38; APG slider, multi-thumb | Not seen in TUIs |
| Date / time picker | 1 (A) | 0 | Datepicker 44; Date input 18 | Rare |
| File picker / directory browser | 3 (T A G) | Y FZ (walker) HX (file picker) | File upload 32 | Common |
| Color picker | 0 (Textual roadmap only) | 0 | Color picker 18 | Not seen |
| Search input with live filter | via List + Input | LG K9 Y BT HX HT BM PO GU FZ AT TV | Search input 30 | Ubiquitous (as `/`) |
| Form container (labels, validation summary, submit) | 1 (G; huh) | 0 seen | Form 21; Fieldset 32 | Rare |
| Stepper / wizard | 0 | 0 | Stepper 20; Ant Steps | Not seen |

### 3.2 Navigation and structure

| Control | F | Apps | DS | Band |
| --- | --- | --- | --- | --- |
| Layout containers (stack, grid, dock, split, layers) | 6 | all | Stack 10 | Ubiquitous |
| Multi-pane / split view | 6 | LG Y BT ZJ HX BM LD PO (8) | APG window splitter; Ant Splitter | Common |
| Resizable splitter | 0 seen | PO (panel resizing); LG has `sidePanelWidth` as config only; ZJ resize unverified | APG window splitter | Rare |
| Tabs | 2 (T G) | Y ZJ PO HX (bufferline) | Tabs 80; APG tabs | Occasional |
| Tree | 4 (T R U G) | BT HT BM PO K9 (xray) | Tree view 14; APG tree view | Common |
| Table / data grid | 4 (T R A G) | K9 BM BT HT | Table 73; APG grid, table, treegrid | Common |
| Breadcrumb | 1 (G) | K9 | Breadcrumbs 55; APG breadcrumb | Rare |
| Collapsible / accordion | 1 (T) | PO (JSON tree) | Accordion 101; APG accordion, disclosure | Rare |
| Content switcher / card stack | 2 (T G) | tabs-based in apps | Carousel 22 | Occasional |
| Pagination | 0 | 0 (scrolling used) | Pagination 48 | Not seen |
| Separator / rule | 5 (T R P U A) | all (box borders) | Separator 34 | Ubiquitous |
| Bordered frame / panel | 5 (R P U A T) | LG (border styles) BT BM | Card 77 | Ubiquitous |
| Scrollbar / scroll gutter | 3 (T P U) | not noted | n/a | Common |
| Header / title bar | 2 (T U) | K9 (header with logo) | Header 38 | Occasional |

### 3.3 Overlays, menus, feedback

| Control | F | Apps | DS | Band |
| --- | --- | --- | --- | --- |
| Modal dialog (confirm / message) | 4 (T P A G; U via Overlay) | K9 Y LG | Modal 82; APG dialog, alert dialog | Common |
| Popup menu / action menu | 4 (P A G U) | LG BT Y HX K9 (5) | Dropdown menu 49; APG menu, menu button | Common |
| Context menu | 0 | 0 seen | Not in component.gallery's 60, Ant's overview, or APG's 30 (Radix's component list was not captured, unverified) | Not seen |
| Popover / drawer | 1 (G floating) | 0 | Popover 50; Drawer 38 | Rare |
| Command palette | 2 (T G) | HX PO | Not in component.gallery's 60, Ant's overview, or Atlassian's index | Occasional |
| Fuzzy picker with preview | 1 (T command palette matcher; P FuzzyCompleter) | HX FZ TV AT LG K9 (6) | Combobox 37 | Common |
| Which-key / pending-chord hint popup | 0 (G keybindbar is a bar) | HX (`auto-info`) Y (`[which]`) | n/a | Occasional |
| Key-hint footer / status bar | 3 (T P G) | HX (statusline) LG (bottom line); HT function-key bar unverified | Footer 19 | Common |
| Help overlay (`?` / F1) | 1 (T HelpPanel) | LG K9 Y BT GU HT BM (7) | n/a | Common |
| Toast / notification | 2 (T G) | Y (`notify`) | Toast 41; Atlassian Flag | Occasional |
| Tooltip | 2 (T G) | 0 | Tooltip 74; APG tooltip | Occasional |
| Alert / inline banner | 0 as widget | BT (paused banner; weak) | Alert 108; Atlassian Banner/Inline message/Section message | Not seen as widget |
| Badge / tag / chip | 0 as widget | 0 documented in app pages; GitHub CLI convention renders labels in parentheses/gray [GH-comp] | Badge 123; Atlassian Lozenge/Tag | Rare |
| Empty state | 1 (G `empty`) | gh CLI guidance | Empty state 16 | Rare |
| Skeleton / placeholder | 1 (T `Placeholder`, design-time) | 0 | Skeleton 30 | Rare |
| Spinner / loading | 3 (T R G) | 0 documented (gh CLI swaps for text) | Spinner 66; Progress indicator 38 | Common |
| Progress bar | 4 (T R P G) | Y (task manager) | Progress bar 40; APG meter | Common |
| Task / job list | 0 | Y (`w`) | n/a | Rare |
| Sparkline / graph / meter | 2 (T G) | BT BM LD (HT meters unverified) | Ant Statistic | Occasional |
| Log / scrolling pane | 1 (T) | LD K9 LG (command log) | n/a | Occasional |
| Hyperlink | 2 (T G) | 0 | Link 64 | Occasional |
| Syntax-highlighted code view | 3 (T R P) | PO HX Y LG | n/a | Common |
| Markdown / rich text render | 3 (T R G) | PO (response bodies; weak) | Rich text editor 5 | Common |
| Image / preview | 1 (G) | Y FZ | Image 29 | Occasional |
| Big digits / banner text | 1 (T; U unverified) | 0 | Hero 9 | Rare |
| JSON / pretty-print viewer | 3 (T R G) | PO | n/a | Common |

### 3.4 Cross-cutting capabilities (treat as framework features, not widgets)

| Capability | F | Apps | Band |
| --- | --- | --- | --- |
| Theme / palette system | 6 | BT K9 GU Y TV PO HQ LG FZ BM (10) | Ubiquitous |
| Mouse support | 4 verified (T P A G; U unverified) | LG LD BT Y HX PO BM K9 FZ (9) | Ubiquitous |
| Custom key bindings | 3 (T P G) | LG GU HX PO HQ ZJ FZ TV K9 (9) | Common |
| Plugin / extension model | 1 (T command providers) | Y K9 ZJ LG LD TV (6) | Common |
| Light/dark auto-adaptation | 1 (T ansi themes) | GU LG HX Y ZJ (5) | Common |
| Vim-style navigation | 0 (apps bind it) | Y HX GU BT (4; LG likely, unverified) | Occasional |
| Snapshot / pilot testing | 2 (T G) | 0 documented | Occasional |
| Dev tools (console, live reload) | 1 (T) | 0 | Rare |
| Jump labels | 1 (G `jumper`) | HX (`gw`) PO | Occasional |
| Clipboard integration | 0 | HX (OSC 52 `termcode`) Y (copy path) | Occasional |
| Read-only / safe mode | 0 | K9 (`--readonly`) | Rare |
| Image protocols | 0 | Y FZ | Occasional |
| Modal editing / mode indicator | 0 | HX ZJ | Occasional |

Takeaways for the gap analysis. (1) Web design systems' most-replicated
components (Badge 123, Button 118, Alert 108, Accordion 101, Radio 85, Checkbox
84, Select 82, Modal 82, Tabs 80, Card 77, Tooltip 74, Table 73) [DS-gallery]
map to TUIs unevenly: Badge and Alert have **no dedicated widget** in Textual,
prompt_toolkit or urwid (they are styled `Static` / labels). (2) Slider,
Stepper, Pagination, Avatar, Rating, Carousel and Color picker are absent from
every surveyed TUI framework and app; they are poor fits for terminals.
(3) The controls users actually meet in well-regarded apps are lists, filters,
panes, help overlays, menus, themes, mouse and key customisation; Command
palette and which-key hints are rarer than expected (2 of 15 each).

## 4. (c) Conventions, accessibility, and terminal-capability checklist

### 4.1 Keyboard and interaction conventions seen across apps

| Convention | Evidence | Src |
| --- | --- | --- |
| `?` opens contextual key help; F1 as fallback | lazygit `?` options menu, k9s `?`, bottom `?`, btop help menu, gitui context help, htop F1/`h`; **yazi uses `~`/F1 for help while `?` is "find previous"** | [LG-cfg] [K9-cmd] [BM-repo] [BT-repo] [GU-repo] [HT-repo] [Y-start] |
| `/` filters or searches, `n`/`N` step through matches, Esc clears | lazygit, k9s (regex, `!` invert, `-f` fuzzy), yazi (`/`, `?`, `n`, `N`, `f`), helix, htop (`/` search, `\` filter), less | [LG-repo] [K9-cmd] [Y-start] [HX-keymap] [HT-repo] [C-less] |
| `:` enters a command line, but its meaning differs per app | lazygit shell prompt, k9s resource switcher (`:pod @ctx ns`), helix command prompt, yazi shell command | [LG-cfg] [K9-cmd] [HX-keymap] [Y-start] |
| Vim motions (`hjkl`, `gg`/`G`) optional or default | yazi and helix default; btop `vim_keys` off by default (it moves help/kill to Shift); gitui ships a vim-style key file; less `j/k/g/G` | [Y-start] [HX-keymap] [BT-repo] [GU-repo] [C-less] |
| `q` quits, Esc backs out one level, Ctrl-C always works | yazi `q`, less `q`, k9s Esc/`:q`/Ctrl-C, fzf Esc/Ctrl-C/Ctrl-G, helix Esc returns to Normal, clig "let the user escape" | [Y-start] [C-less] [K9-cmd] [FZ-repo] [HX-keymap] [CLIG] |
| Lower/upper case pairs encode safe/reverse or destructive variants | yazi `d` trash vs `D` permanent, sort lowercase asc / uppercase reverse; k9s sort via Shift+letter | [Y-start] [K9-repo] |
| Confirmation scaled to danger, with a fast path for experts | k9s `ctrl-d` confirms (TAB then ENTER) vs `ctrl-k` no confirm; yazi `[confirm]` dialogs for trash/delete/overwrite/quit; clig mild/moderate/severe ladder, type-the-name for severe | [K9-cmd] [Y-cfg] [CLIG] |
| Prefix keys / chords, sometimes with a hint popup | yazi `c`+letter, `,`+sort letter, `[which]` popup; helix `g`, `z`, Space, Ctrl-w minor modes; zellij modal `SwitchToMode`; which-key.nvim default delay 200 ms, "hydra" loop mode | [Y-start] [Y-cfg] [HX-keymap] [ZJ-keys] [WK-repo] |
| Picker keys: Ctrl-n/p or Ctrl-j/k move, Enter accepts, Tab multi-select or cycles, Alt-Enter background, Ctrl-t preview toggle | helix pickers, fzf | [HX-keymap] [FZ-repo] |
| Readline/Emacs editing in text fields | Textual Input and TextArea (ctrl+a/e/k/u/w/d), helix prompt, prompt_toolkit default; note Textual maps ctrl+f to delete-word-right, unlike readline's forward-char | [T-inputw] [T-textarea] [HX-keymap] [PT-input] [C-readline] |
| Ctrl-R history search; Ctrl-T file insert; Alt-C cd | fzf, atuin, television shell integrations | [FZ-repo] [AT-repo] [TV-repo] |
| Numeric hotkeys jump to tabs or recent items | yazi `1`-`9`, atuin Alt+number, btop presets | [Y-start] [AT-repo] [BT-repo] |
| Space toggles selection or expansion | yazi, k9s row mark, Textual SelectionList and Tree | [Y-start] [K9-repo] [T-sellist] [T-tree] |
| Ctrl-P is overloaded | Textual command palette vs lazygit custom patch options | [T-palette] [LG-repo] |
| Pager keys | less: Space/b/d/u/j/k/g/G, `/` `?` `n` `N`, `&pattern` filter, `F` follow, `v` editor, `-R -F -X -I`; clig recommends `less -FIRX` | [C-less] [CLIG] |
| Shell-wrapper "cd on exit" | lazygit `LAZYGIT_NEW_DIR_FILE`, yazi `y` wrapper with `--cwd-file`, `Q` quits without cd | [LG-repo] [Y-start] |
| Fullscreen vs inline | fzf `--height N%` renders inline; Textual `is_inline` / `INLINE_PADDING`; Rich `Live(screen=True)` | [FZ-repo] [T-appapi] [R-live] |
| Suspend / editor hand-off | Textual `action_suspend_process` (SIGTSTP) and suspend signals; less `v`; posting opens editor/pager | [T-appapi] [C-less] [PO-readme] |
| Mouse optional, with selection escape hatch | lazygit `mouseEvents` (capturing hurts text selection); lazydocker option-drag / `gui.ignoreMouseEvents`; Textual FAQ Shift-drag; yazi `mouse_events` list (empty disables); helix `mouse = true` | [LG-cfg] [LD-repo] [T-faq] [Y-cfg] [HX-editor] |
| Live data views refresh in place with configurable rate; a read-only mode exists for ops tools | k9s | [K9-repo] |

WAI-ARIA keyboard rules that map onto TUIs [DS-apgkb]: Tab enters a composite
once and arrows move inside it; focus must never be lost (return it to the
trigger when a dialog closes or to the next item after a delete); selection must
look different from focus; selection may follow focus only when switching is
instant; use "disabled but focusable" for items users should still discover
(Textual's `check_action` returning `None` renders the key dim, equivalent) [T-actions];
initial focus lands on the selected item in radio/tab/list/tree and the first
item in menubar/toolbar; shortcuts supplement and never replace navigation.

### 4.2 Output, colour, and accessibility

| Rule | Source detail | Src |
| --- | --- | --- |
| Honor `NO_COLOR` (non-empty, any value); flags and user config may override; it removes colour only, not bold/underline | | [NOCOLOR] [R-console] |
| Also honor `FORCE_COLOR`, `TERM=dumb`, `CLICOLOR=0`, `CLICOLOR_FORCE`, `--no-color`, and non-TTY streams (check stdout and stderr separately) | Rich: constructor args beat env; `TTY_COMPATIBLE` / `TTY_INTERACTIVE` for CI | [CLIG] [R-console] [GH-env] |
| No animations or progress bars when output is not a TTY | | [CLIG] [R-console] |
| Respond within 100 ms; show progress for slow work; a second Ctrl-C skips slow cleanup | | [CLIG] |
| Colour must not be the only carrier of meaning; use glyph plus word (✓ success, ✗ failure, ! alert, - neutral, + changes requested) | GitHub CLI Primer | [GH-found] |
| Use the 8 basic ANSI colours (bright is less dependable); do not assume 256; users remap the 16 ANSI colours via their terminal theme | GitHub CLI Primer, GitHub blog | [GH-found] [GH-blog] |
| Hierarchy via weight and whitespace; italics avoided; end phrases with punctuation so screen readers pause | | [GH-found] |
| Accessible mode: 4-bit palette tied to terminal theme, static text instead of an animated spinner, screen-reader-friendly prompter; env vars `GH_ACCESSIBLE_COLORS`, `GH_ACCESSIBLE_PROMPTER`, `GH_SPINNER_DISABLED`; `gh a11y` enables | Problems found: braille spinner and redrawn prompts are unreadable for screen readers; palette ignored terminal background | [GH-blog] [GH-env] |
| Forms offer a non-TUI accessible path (`WithAccessible(true)`, suggested `ACCESSIBLE` env var) | huh | [HUH] |
| Non-text contrast >= 3:1 for UI component boundaries and focus indicators; user-modified focus indicators must meet it | WCAG 1.4.11 (the fetched page did not state the 4.5:1 text figure; unverified here) | [WCAG-ntc] |
| Radix scale targets APCA Lc 60 / Lc 90 for step 11 / 12 text on step 2 backgrounds | | [DS-radix-scale] |
| Screen readers track the hardware cursor; Textual hides it, and the community issue asks to keep it where focus is | Community claim in the issue; maintainer says terminals have "historically poor accessibility" | [T-a11y] |
| Reduced motion: **no terminal-level `prefers-reduced-motion` equivalent found**; frameworks expose app-level switches (Textual `animation_level` / `TEXTUAL_ANIMATIONS`, gh `GH_SPINNER_DISABLED`); Atlassian lists reduced motion and density as theme dimensions | | [T-appapi] [GH-env] [DS-atl-tokens] |
| ASCII / reduced-glyph fallbacks | Rich `Table(safe_box=True)`; btop TTY mode (`-t`, TTY theme, tty graph symbols); lazygit empty `nerdFontsVersion` = no icons; urwid mono mode; prompt_toolkit `ColorDepth.MONOCHROME`; Primer notes PowerShell default font handles Unicode poorly | [R-table] [BT-repo] [LG-cfg] [UR-attr] [PT-style] [GH-found] |
| Machine output differs from terminal output: no colour, spelled-out states, tab-separated, no truncation, no header | | [GH-found] [CLIG] |
| Palette choice trade-off: Textual emits truecolor everywhere (consistent look, blends and shades) and added `ansi-dark` / `ansi-light` themes and `App.ansi_color` in 8.2.5 to opt into the user's palette; GitHub CLI and lazygit choose the user's ANSI palette | | [T-faq] [T-rel] [GH-found] [LG-cfg] |
| Screen-reader and colour-blind features in Textual are on the roadmap, not shipped (as far as the fetched pages show) | | [T-road] [T-a11y] |

### 4.3 Terminal-capability checklist (detect, enable, fall back)

| Capability | How to use | Detect / fall back | Evidence | Src |
| --- | --- | --- | --- | --- |
| Truecolor | SGR 38/48;2 | `COLORTERM=truecolor`/`24bit` (not forwarded by ssh/sudo by default); optional DECRQSS probe; terminfo `RGB`; else 256 then 16; approximate with CIEDE2000 | helix `true-color` override; btop 24-bit then 256 then 16/TTY; prompt_toolkit defaults to 256 unless `ColorDepth.TRUE_COLOR` | [C-color] [HX-editor] [BT-repo] [PT-style] |
| Dark/light detection | OSC 10/11 queries; `CSI ? 996 n` reply 997;1 dark / 997;2 light; DEC mode 2031 change notifications | Adopted by Contour, Ghostty, Kitty, Neovim, Helix, Zellij 0.44.2, VTE 0.82, Yazi; not yet lazygit, WezTerm, Vim, Nushell | | [C-darklight] [C-xterm] |
| Kitty keyboard protocol | `CSI > flags u` push, `CSI < u` pop; flags 1 disambiguate, 2 event types (press/repeat/release), 4 alternate keys, 8 all keys as escapes, 16 associated text | Query `CSI ? u` plus DA1 trick; crossterm exposes `PushKeyboardEnhancementFlags` and `supports_keyboard_enhancement`; Textual 8.2.7 added fuller support and `TEXTUAL_DISABLE_KITTY_KEY` | Adopters: kitty, alacritty, foot, ghostty, iTerm2, WezTerm, Warp, xterm.js; libs crossterm, textual, bubbletea; apps Helix, Yazi, Neovim | [C-kitty] [C-crossterm-ev] [C-crossterm-term] [T-rel] |
| Esc vs escape-sequence ambiguity | Timeout heuristics | prompt_toolkit `ttimeoutlen`; kitty flag 1 removes the ambiguity | | [PT-keys] [C-kitty] |
| Mouse | DECSET 1000 / 1002 / 1003 plus **1006 SGR** encoding (X10 caps at col 223); 1016 pixel coordinates; wheel buttons; modifiers | Mouse is opt-in in crossterm (`EnableMouseCapture`); users need an escape hatch (Shift/Option-drag) | | [C-xterm] [C-crossterm-ev] [T-faq] |
| Bracketed paste | `CSI ? 2004 h/l` | crossterm `EnableBracketedPaste` yields `Event::Paste` (feature-gated) | | [C-xterm] [C-crossterm-ev] |
| Focus events | `CSI ? 1004 h`; `CSI I` / `CSI O` | crossterm `EnableFocusChange` | | [C-xterm] [C-crossterm-ev] |
| Synchronized updates | `CSI ? 2026 h` ... `CSI ? 2026 l` | Query with DECRQM `CSI ? 2026 $ p`; reply 1 or 2 = supported, 0 or 4 = not; crossterm has Begin/EndSynchronizedUpdate but no support-query helper | Used by tmux, neovim, Textual | [C-sync] [C-crossterm-term] |
| Clipboard (OSC 52) | `OSC 52 ; c ; <base64> ST`; `?` queries | Often gated by `allowWindowOps`; helix `clipboard-provider = "termcode"` | Security posture of query support is unverified | [C-xterm] [HX-editor] |
| Hyperlinks (OSC 8) | `OSC 8 ; id=... ; URI ST` ... `OSC 8 ; ; ST` | **No query**; target must not be reachable only via the link; URIs capped near 2083 bytes; full-screen apps should set explicit ids | | [C-osc8] |
| Unicode width and grapheme clusters | Compute cell width per grapheme | Contour's unicode-core proposal (alpha) defines a mode for grapheme-aware width (details not fetched, unverified); Primer warns font coverage varies; Textual FAQ notes macOS Terminal.app misaligned box glyphs | | [C-unicode] [GH-found] [T-faq] |
| Alternate screen | DECSET 1049 | Rich `Console.screen()` restores on exit; `reset` recovers a stuck terminal | | [C-xterm] [R-console] |
| Image protocols | Kitty, Sixel, iTerm2, Unicode-block fallback (Chafa) | Per-terminal list in yazi README | | [Y-repo] [FZ-repo] |
| Size and env | `COLUMNS`/`LINES`, window size query | Rich honors `COLUMNS`/`LINES` | | [R-console] [CLIG] |

## 5. (d) Design-system ideas worth borrowing

### 5.1 Semantic colour tokens

- **Textual**: 11 base tokens (`$primary $secondary $accent $foreground
  $background $surface $panel $boost $warning $error $success`), 6 shade steps
  per colour (`-lighten-1..3`, `-darken-1..3`), `-muted` variants (colour at 70%
  over `$background`), text tokens (`$text`, `$text-muted`, `$text-disabled`, and
  `$text-primary` ... tinted to stay legible on surface/panel/background), and
  per-component families (`$footer-*`, `$button-*`, `$input-*`, `$scrollbar-*`,
  `$link-*`, `$block-cursor-*`). Themes need only `primary`; the rest is derived.
  `textual colors` previews the system [T-design].
- **Primer**: foundational prefixes `fgColor-*`, `bgColor-*`, `borderColor-*`,
  `shadow-*`; roles default/muted/accent/danger/success/attention/neutral/severe;
  emphasis vs muted intensity for bg/border; `onEmphasis` helpers; pattern tokens
  follow `component-variant-property-state` (`button-primary-bgColor-hover`) with
  states rest/hover/active/disabled/selected [DS-primer-color].
- **Atlassian**: `foundation.property.modifier` (for example `color.text`,
  `color.icon.success`); "pick tokens for meaning, not looks, or other themes
  break"; a theme is a token-value set that can also vary density and reduced
  motion [DS-atl-tokens].
- **Radix** 12-step scale: 1-2 backdrops, 3 resting component fill, 4 hover, 5
  pressed/selected, 6-8 borders (8 = focus ring), 9-10 solid fills, 11-12 text
  with APCA guarantees [DS-radix-scale].
- Idea: a token table keyed by role and state, with derived `muted`/`disabled`
  text and cursor-blurred variants, and a preview command (`textual colors`).

### 5.2 States: hover, focus, disabled, selected

- Textual pseudo-classes: `:hover :focus :blur :focus-within :disabled :enabled
  :dark :light :inline :empty :first-child :last-child :even :odd` [T-css].
  Cursor tokens distinguish focused vs blurred (`$block-cursor-blurred-*` at 30%)
  and hover (`$block-hover-background` = `$boost` at 5%) [T-design].
- Availability as a three-state value: `check_action` returns True (show+run),
  False (hide), None (show dim, blocked); `refresh_bindings()` or
  `reactive(..., bindings=True)` re-evaluates [T-actions].
- Selection distinct from focus (APG) [DS-apgkb]; urwid `AttrMap(focus_map=...)`
  as the minimal focus-state hook [UR-attr].
- `Widget.loading`, `disabled` (inherited via ancestors: `is_disabled`) [T-widgetapi].

### 5.3 Density

- Textual `compact` flag on Footer, OptionList, Select, SelectionList, TextArea
  (removes borders; Footer adds `-compact`) [T-footer] [T-optlist] [T-select]
  [T-sellist] [T-textarea]; lazygit `screenMode` normal/half/full and
  `sidePanelWidth`/`expandFocusedSidePanel` [LG-cfg]; bottom "basic mode" [BM-repo];
  Atlassian density as a theme axis [DS-atl-tokens]; btop presets and box toggles
  [BT-repo]; Textual `ALLOW_MAXIMIZE` for single-pane zoom [T-widgetapi].

### 5.4 Key-binding data to help generation

- Textual `Binding(key, action, description, show, key_display, priority,
  tooltip, id, system, group)`; `id` lets a user keymap replace the key
  (`BindingsMap.apply_keymap` reports `clashed_bindings`); `Binding.Group(compact,
  description)` groups related keys in the footer; `system` keeps entries out of
  the key panel [T-binding]. The same action strings drive footer, tooltips,
  help panel, and (via `get_system_commands`) the command palette [T-footer]
  [T-help] [T-palette].
- Help text is attached to widgets, not only keys: `HELP` markdown on a widget is
  surfaced in a side panel for whichever widget is focused [T-help].
- Command palette providers: `startup`, `search(query)` yields scored `Hit`s,
  `discover()` supplies the empty-query list, `shutdown`; fuzzy `Matcher.match`
  and `highlight`; per-screen `COMMANDS` scope the list to the active screen
  [T-palette].
- prompt_toolkit: key bindings take `filter=` conditions and `eager`, support
  `ConditionalKeyBindings` and `merge_key_bindings`, and show a `bottom_toolbar`
  of hints [PT-keys] [PT-prog].
- Terma has a dedicated `keybindbar` widget and `commandpalette` [TM-widgets].
- Mode-aware hinting: Helix `auto-info`, Yazi `[which]`, which-key.nvim (delay,
  groups, hydra loop) [HX-editor] [Y-cfg] [WK-repo].

### 5.5 Overlay and screen contracts

- `ModalScreen` blocks app-level bindings and dims the screen beneath;
  `dismiss(value)` plus a typed callback decouples dialog from caller;
  `push_screen_wait` for async flows; `MODES` for independent stacks; Suspend /
  Resume events [T-screens].
- prompt_toolkit `modal=True` containers stop key-binding inheritance but global
  bindings still apply [PT-full].
- APG: return focus to the trigger on close [DS-apgkb].

### 5.6 Snapshot and interaction testing

- Textual `run_test()` Pilot (`press`, `click(selector, offset, times)`, `hover`,
  `pause`), `snap_compare(app, press=[...], terminal_size=(w, h),
  run_before=async fn)`, SVG baselines, HTML report with overlay diff,
  `--snapshot-update` [T-test] [T-snap]. Textual 8.2.5 notes that theme changes
  may break snapshots without any visible difference [T-rel].
- Rich: `Console(record=True)` + `export_svg()` / `export_text()`; `capture()` or
  a `StringIO` file console for unit tests [R-console].
- Terma ships `testing.md` and `browser-testing.md` pages [TM-docs].

### 5.7 Developer ergonomics

- Live CSS reload, remote console, key inspector (`textual keys`), colour
  preview, border preview, easing preview [T-dev].
- Placeholder widget for layout sketches [T-gallery].
- Trogon: derive a TUI form from a CLI schema [T-trogon].

### 5.8 CLI-adjacent rules worth importing

- GitHub CLI Primer components: syntax conventions (branches in brackets/cyan,
  labels in parentheses/gray, repositories bold), prompts always have a matching
  flag, headers orient the user, lists are tables with a header, detail views
  indent the body with the URL last, empty states carry a message, help pages
  have fixed sections [GH-comp]. Help-syntax grammar: `<placeholder>`,
  `[optional]`, `{a | b}`, trailing `...` [GH-comp].
- Piped output rules and exit-time behaviour [GH-found] [CLIG].

## 6. Candidate additions suggested by the evidence (for the gap analysis to weigh)

These are observations, not recommendations. Conspectus already has file-level
widgets named `badge`, `form`, `help`, `input`, `multi_select`, `picker`,
`popup_frame`, `row_menu`, `toast`, `path_omnibox`, `pane_viewer`, `value_modal`
(from `src/tui/widgets/`, not audited).

| Candidate | Why the evidence suggests it | Commonness |
| --- | --- | --- |
| Named-action command palette (all actions searchable with help text, scoped per screen) | Textual, Terma, Helix, Posting have one; built from the same action table as footer/help | F 2/6, Apps 2/15 |
| Which-key / pending-chord popup | Helix, Yazi; reduces key-memorisation cost for chords | Apps 2/15 |
| Tri-state action availability (show / hide / dim) | Textual `check_action`; ties help and footer to current state | Textual only |
| Collapsible section / accordion | Textual, APG, web systems (Accordion 101) | F 1/6 |
| Tooltip on key hints | Textual `Binding.tooltip`; Terma | F 2/6 |
| Switch / toggle distinct from checkbox | Textual; Toggle 60 on the web | F 1/6 |
| Sparkline / inline graph | Textual, Terma; btop, bottom, lazydocker use graphs | F 2/6, Apps 3/15 |
| Background task list | Yazi `w` tasks manager | Apps 1/15 |
| Jump labels | Helix `gw`, Posting jump mode, Terma `jumper` | Apps 2/15, F 1/6 |
| Non-colour status carriers + accessible mode (static progress text, plain prompts) | GitHub CLI work and huh; clig | Guidance, not a widget |
| Light/dark auto-adaptation via mode 2031 | Helix, Yazi, Zellij, Neovim support it | Apps 5/15 |
| Date/time picker, slider, stepper, pagination, color picker | Present in web systems and asciimatics only | Rare; likely skip |
| Empty-state and skeleton conventions | gh CLI empty-state rule; Skeleton 30 on the web | Rare as widgets |
| Grouped footer hints (`Binding.Group`, `combine_groups`) | Textual | Textual only |

## 7. Things I could not verify (explicit list)

- Textual: whether any release exists after 8.2.8 (2026-06-30); accepted values
  of `TEXTUAL_ANIMATIONS`; doc pages for `HelpPanel`/`KeyPanel`/`Tooltip`;
  whether Footer keys are clickable (the Footer page was silent; the key-binding
  guide said entries are clickable); `HORIZONTAL_BREAKPOINTS` (not found).
- Rich Syntax/Markdown/JSON option detail; Rich `Tree` options `hide_root`,
  `expanded`; ASCII tree guides.
- urwid BigText/ProgressBar reference entries; multi-line `Edit`.
- Trogon release year; textual-web last commit; asciimatics last commit.
- All per-app statements not present in the fetched pages, notably: zellij
  status-bar hints and resize mode, htop function-key bar and mouse, lazygit
  `?` popup contents and tabs, gitui popups and tabs, television multi-select
  and help panel, harlequin command palette, posting footer, k9s `0`-`9`
  shortcuts and hotkey syntax.
- Material 3 component list and token tiers (page returned no body); Radix
  Primitives component list (nav not captured); Primer colour-blind / high
  contrast theme delivery; GitHub CLI Primer accessibility sub-pages
  (the `foundations` page covered colour and symbols only).
- Contour mode 2027 details and any terminal list; whether OSC 52 query is
  commonly disabled by terminals (security posture).
- WCAG 4.5:1 text threshold (fetched page did not restate it).
- Helix has no release newer than 25.07.1 in the page (older than a year);
  gitui release years not shown.
- Terma's maturity: public repo shows 4 stars and no releases, so treat its
  widget list as a feature inventory only.

## 8. Notable discoveries outside the assigned scope

- Posting 3 rewrote from Textual (Python) to Go with Terma, a TUI library by the
  same author as Textual's co-lead; Terma's docs enumerate a widget set that
  includes `commandpalette`, `keybindbar`, `jumper`, `focustrap`, `filepicker`,
  `jsonviewer`, `presentedtext`, `empty`, and `breadcrumbs`. Another researcher
  covering Go frameworks may want Terma [TM-widgets] [PO-readme].
- GitHub's blog documents a concrete accessible-mode design for a CLI/TUI-ish
  prompter and spinner [GH-blog].

## 9. Source index

### Textual and Python ecosystem

| Key | URL |
| --- | --- |
| T-gallery | https://textual.textualize.io/widget_gallery/ |
| T-widgets | https://textual.textualize.io/widgets/ |
| T-pypi | https://pypi.org/project/textual/ |
| T-rel | https://github.com/Textualize/textual/releases |
| T-chg | https://raw.githubusercontent.com/Textualize/textual/main/CHANGELOG.md |
| T-repo | https://github.com/Textualize/textual |
| T-design | https://textual.textualize.io/guide/design/ |
| T-input | https://textual.textualize.io/guide/input/ |
| T-palette | https://textual.textualize.io/guide/command_palette/ |
| T-test | https://textual.textualize.io/guide/testing/ |
| T-screens | https://textual.textualize.io/guide/screens/ |
| T-actions | https://textual.textualize.io/guide/actions/ |
| T-binding | https://textual.textualize.io/api/binding/ |
| T-footer | https://textual.textualize.io/widgets/footer/ |
| T-help | https://raw.githubusercontent.com/Textualize/textual/main/src/textual/widgets/_help_panel.py |
| T-tip | https://raw.githubusercontent.com/Textualize/textual/main/src/textual/widgets/_tooltip.py |
| T-toast | https://raw.githubusercontent.com/Textualize/textual/main/src/textual/widgets/_toast.py |
| T-css | https://textual.textualize.io/guide/CSS/ |
| T-widgetapi | https://textual.textualize.io/api/widget/ |
| T-appapi | https://textual.textualize.io/api/app/ |
| T-faq | https://textual.textualize.io/FAQ/ |
| T-react | https://textual.textualize.io/guide/reactivity/ |
| T-dev | https://textual.textualize.io/guide/devtools/ |
| T-layout | https://textual.textualize.io/guide/layout/ |
| T-anim | https://textual.textualize.io/guide/animation/ |
| T-textarea | https://textual.textualize.io/widgets/text_area/ |
| T-inputw | https://textual.textualize.io/widgets/input/ |
| T-datatable | https://textual.textualize.io/widgets/data_table/ |
| T-optlist | https://textual.textualize.io/widgets/option_list/ |
| T-tree | https://textual.textualize.io/widgets/tree/ |
| T-select | https://textual.textualize.io/widgets/select/ |
| T-sellist | https://textual.textualize.io/widgets/selection_list/ |
| T-collapsible | https://textual.textualize.io/widgets/collapsible/ |
| T-masked | https://textual.textualize.io/widgets/masked_input/ |
| T-future | https://textual.textualize.io/blog/2025/05/07/the-future-of-textualize/ |
| T-road | https://github.com/Textualize/textual/blob/main/docs/roadmap.md |
| T-a11y | https://github.com/Textualize/textual/issues/2425 |
| T-ai | https://github.com/Textualize/textual/blob/main/AI_POLICY.md |
| T-snap | https://pypi.org/project/pytest-textual-snapshot/ |
| T-devpkg | https://pypi.org/project/textual-dev/ |
| T-serve | https://pypi.org/project/textual-serve/ |
| T-trogon | https://github.com/Textualize/trogon |
| T-trogon-rel | https://github.com/Textualize/trogon/releases |
| T-web | https://github.com/Textualize/textual-web |
| R-pypi | https://pypi.org/project/rich/ |
| R-ref | https://rich.readthedocs.io/en/latest/reference.html |
| R-intro | https://rich.readthedocs.io/en/latest/introduction.html |
| R-live | https://rich.readthedocs.io/en/latest/live.html |
| R-console | https://rich.readthedocs.io/en/latest/console.html |
| R-progress | https://rich.readthedocs.io/en/latest/progress.html |
| R-layout | https://rich.readthedocs.io/en/latest/layout.html |
| R-prompt | https://rich.readthedocs.io/en/latest/prompt.html |
| R-table | https://rich.readthedocs.io/en/latest/tables.html |
| R-tree | https://rich.readthedocs.io/en/latest/tree.html |
| R-columns | https://rich.readthedocs.io/en/latest/columns.html |
| PT-pypi | https://pypi.org/project/prompt-toolkit/ |
| PT-repo | https://github.com/prompt-toolkit/python-prompt-toolkit |
| PT-ref | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/reference.html |
| PT-full | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/full_screen_apps.html |
| PT-input | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/asking_for_input.html |
| PT-dialogs | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/dialogs.html |
| PT-prog | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/progress_bars.html |
| PT-keys | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/advanced_topics/key_bindings.html |
| PT-style | https://python-prompt-toolkit.readthedocs.io/en/stable/pages/advanced_topics/styling.html |
| PT-widgets | https://raw.githubusercontent.com/prompt-toolkit/python-prompt-toolkit/main/src/prompt_toolkit/widgets/__init__.py |
| PT-short | https://raw.githubusercontent.com/prompt-toolkit/python-prompt-toolkit/main/src/prompt_toolkit/shortcuts/__init__.py |
| UR-pypi | https://pypi.org/project/urwid/ |
| UR-repo | https://github.com/urwid/urwid |
| UR-widgets | https://urwid.org/manual/widgets.html |
| UR-ref | https://urwid.org/reference/widget.html |
| UR-attr | https://urwid.org/manual/displayattributes.html |
| AS-pypi | https://pypi.org/project/asciimatics/ |
| AS-repo | https://github.com/peterbrittain/asciimatics |
| AS-widgets | https://asciimatics.readthedocs.io/en/stable/widgets.html |
| PG-repo | https://github.com/bczsalba/pytermgui |
| TM-repo | https://github.com/darrenburns/terma |
| TM-docs | https://github.com/darrenburns/terma/tree/main/docs |
| TM-widgets | https://github.com/darrenburns/terma/tree/main/docs/widgets |
| TM-index | https://github.com/darrenburns/terma/blob/main/docs/index.md |
| PO-readme | https://raw.githubusercontent.com/darrenburns/posting/main/README.md |
| HUH | https://github.com/charmbracelet/huh |

### Design systems and guidance

| Key | URL |
| --- | --- |
| DS-gallery | https://component.gallery/components/ |
| DS-ant | https://ant.design/components/overview |
| DS-atl | https://atlassian.design/components |
| DS-apg | https://www.w3.org/WAI/ARIA/apg/patterns/ |
| DS-apgkb | https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/ |
| DS-primer-color | https://primer.style/product/primitives/color/ |
| DS-atl-tokens | https://atlassian.design/foundations/tokens/design-tokens |
| DS-radix-scale | https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale |
| GH-found | https://raw.githubusercontent.com/cli/cli/trunk/docs/primer/foundations/README.md |
| GH-comp | https://raw.githubusercontent.com/cli/cli/trunk/docs/primer/components/README.md |
| GH-blog | https://github.blog/engineering/user-experience/building-a-more-accessible-github-cli/ |
| GH-env | https://cli.github.com/manual/gh_help_environment |
| CLIG | https://clig.dev/ |
| NOCOLOR | https://no-color.org/ |
| WCAG-ntc | https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html |

### Terminal capabilities

| Key | URL |
| --- | --- |
| C-kitty | https://sw.kovidgoyal.net/kitty/keyboard-protocol/ |
| C-color | https://github.com/termstandard/colors |
| C-osc8 | https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda |
| C-sync | https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036 |
| C-unicode | https://github.com/contour-terminal/terminal-unicode-core |
| C-xterm | https://invisible-island.net/xterm/ctlseqs/ctlseqs.html |
| C-darklight | https://contour-terminal.org/vt-extensions/color-palette-update-notifications/ |
| C-crossterm-ev | https://docs.rs/crossterm/latest/crossterm/event/index.html |
| C-crossterm-term | https://docs.rs/crossterm/latest/crossterm/terminal/index.html |
| C-readline | https://tiswww.case.edu/php/chet/readline/readline.html |
| C-less | https://man7.org/linux/man-pages/man1/less.1.html |

### Apps

| Key | URL |
| --- | --- |
| LG-repo | https://github.com/jesseduffield/lazygit |
| LG-cfg | https://raw.githubusercontent.com/jesseduffield/lazygit/master/docs/Config.md |
| LG-rel | https://github.com/jesseduffield/lazygit/releases |
| K9-repo | https://github.com/derailed/k9s |
| K9-cmd | https://k9scli.io/topics/commands/ |
| K9-rel | https://github.com/derailed/k9s/releases |
| Y-repo | https://github.com/sxyazi/yazi |
| Y-start | https://yazi-rs.github.io/docs/quick-start |
| Y-cfg | https://yazi-rs.github.io/docs/configuration/yazi/ |
| Y-rel | https://github.com/sxyazi/yazi/releases |
| BT-repo | https://github.com/aristocratos/btop |
| BT-rel | https://github.com/aristocratos/btop/releases |
| GU-repo | https://github.com/gitui-org/gitui |
| GU-rel | https://github.com/gitui-org/gitui/releases |
| HX-keymap | https://docs.helix-editor.com/keymap.html |
| HX-editor | https://docs.helix-editor.com/editor.html |
| HX-rel | https://github.com/helix-editor/helix/releases |
| ZJ-repo | https://github.com/zellij-org/zellij |
| ZJ-keys | https://zellij.dev/documentation/keybindings.html |
| ZJ-rel | https://github.com/zellij-org/zellij/releases |
| AT-repo | https://github.com/atuinsh/atuin |
| AT-rel | https://github.com/atuinsh/atuin/releases |
| TV-repo | https://github.com/alexpasmantier/television |
| PO-repo | https://github.com/darrenburns/posting |
| HQ-repo | https://github.com/tconbeer/harlequin |
| FZ-repo | https://github.com/junegunn/fzf |
| HT-repo | https://github.com/htop-dev/htop |
| BM-repo | https://github.com/ClementTsang/bottom |
| LD-repo | https://github.com/jesseduffield/lazydocker |
| WK-repo | https://github.com/folke/which-key.nvim |
