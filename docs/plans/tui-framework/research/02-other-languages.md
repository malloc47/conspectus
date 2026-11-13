# Research 02: TUI frameworks outside Rust and Python

Evidence base for the gap analysis (`gap-analysis.md`). Covers Go, JavaScript and
TypeScript, C and C++, Haskell, Zig, Elixir, .NET, JVM, Dart and others. Compiled
2026-10-27 from documentation and source pages fetched over the web. Nothing was
cloned, installed, or executed.

Bracketed tags such as `[BBL-LIST]` are source keys; every key resolves to a URL in the
Sources section at the end. Anything marked **unverified** could not be confirmed from
the fetched pages.

## Method and caveats

- The WebSearch tool was unavailable in this session (the API rejected the tool type).
  Project discovery therefore came from the Charm README ecosystem list `[BT-README]`
  and the `awesome-tuis` library list `[AWESOME]`, plus prior knowledge. Projects named
  only in those lists are marked "not examined".
- The GitHub REST API (403) and the npm registry (connection reset) were not reachable.
  Version and date evidence came from `pkg.go.dev` version tabs, `deps.dev` package
  endpoints, NuGet, Hackage, `pub.dev`, Maven metadata, and GitHub HTML pages.
- Page text was summarised by a small model before I saw it, so exact API spellings
  should be re-checked against the linked page before anyone codes against them.
- GitHub release pages show day and month without a year. Where `deps.dev` or
  `pkg.go.dev` gave a full date, I use that. Otherwise I say the year is inferred.
- "Today" is 2026-10-27. I call a project **active** if it shipped a release or
  version-tagged build in roughly the last 3 months, **quiet** if its last release is
  3 to 18 months old, and **dormant** if older than that.

## 1. Projects examined

| Project | Lang | Latest version seen (date) | Maintenance read | Sources |
| --- | --- | --- | --- | --- |
| Bubble Tea v2 | Go | v2.1.0 (2026-10-08) | Active; roughly monthly releases since 2026-04 | `[BT-VER]` `[BT-REL]` |
| Bubbles v2 | Go | v2.2.1 (2026-08-24) | Active; v2.2.0 added textarea selection and the `tree` package arrived by v2.2.1 | `[BBL-VER]` `[BBL-TA]` `[BBL-TREE]` |
| Lip Gloss v2 | Go | v2.0.6 (2026-08-11) | Active; 7 releases since 2026-02-24 | `[LG-VER]` |
| Huh v2 | Go | v2.0.3 (2026-03-10); v1.0.0 on the old path (2026-02-23) | Quiet (7 months); stable API | `[HUH-VER]` |
| Glamour v2 | Go | v2.0.1 (2026-06-12) | Quiet | `[GLAM-VER]` |
| Fang v2 | Go | v2.0.1 (2026-03-11) | Quiet; README calls it experimental | `[FANG-PKG]` `[FANG-VER]` |
| Harmonica | Go | v0.2.0 (2022-04-15) | Dormant but tiny and finished | `[HARM-VER]` `[HARM-GH]` |
| bubblezone v2 | Go | v2.0.0 (2026-02-28) | Quiet; warns it may not work with the Lip Gloss v2 compositor | `[BZ-VER]` `[BZ-GH]` |
| Ultraviolet | Go | no tags seen | README: "in active development, API may change"; underlies Bubble Tea v2 and Lip Gloss v2 | `[UV-GH]` |
| VHS | Go | v0.12.1 (2026-09-24) | Active | `[VHS-DEPS]` `[VHS-GH]` |
| Wish v2 | Go | v2.0.5 (2026-10-01) | Active; v2.0.0 flagged by a Go advisory for SCP path traversal | `[WISH-VER]` |
| Charm `x` (teatest, golden) | Go | teatest v2 pseudo-version 2026-10-25; golden v0.1.0 | Experimental, "no promises of backwards compatibility" | `[X-GH]` `[TEATEST2]` `[GOLDEN]` |
| gum | Go | v2 module path; version not shown | Charm-maintained; version unverified | `[GUM-GH]` |
| Crush | Go | not shown | Charm product; used as a reference app only | `[CRUSH-GH]` |
| ntcharts v2 | Go | v2.8.0 (2026-10-11) | Active | `[NTC-DEPS]` `[NTC-GH]` |
| stickers | Go | v1.5.0 (2025-09-28) | Quiet; commits to 2026-04 | `[STICK-DEPS]` `[STICK-GH]` |
| Evertras bubble-table | Go | version not shown | Bubble Tea v2 support unverified | `[EVT-GH]` |
| pterm | Go | v0.12.84 (2026-10-10) | Active; output-oriented, not a full TUI | `[PTERM-DEPS]` `[PTERM-GH]` |
| tview | Go | v0.42.0 (2025-08-27) | Quiet (14 months since last tag); backward compatibility is a stated goal; newer pseudo-versions unverified | `[TVIEW-VER]` `[TVIEW-GH]` |
| gocui (jesseduffield) | Go | no semver tag since v0.3.0 (2016); commits to 2026-04 | Maintained as lazygit's engine | `[GOCUI-DEPS]` `[GOCUI-GH]` |
| Ink | TS | 8.0.0 (2026-10-03) | Active; 40k stars; used by Claude Code, Gemini CLI, others | `[INK-DEPS]` `[INK-GH]` |
| @inkjs/ui | TS | 2.0.0 (2024-05-22) | Dormant (last commit 2024-05) | `[INKUI-DEPS]` `[INKUI-TAGS]` |
| Ink ecosystem | TS | ink-testing-library 4.0.0 (2024-05); ink-select-input 6.2.0 (2025-04); ink-text-input 6.0.0 (2024-05); ink-table 3.1.0 (2023-12); ink-spinner 5.0.0 (2023-03); ink-scroll-view 0.4.0 (2026-09); ink-form 2.0.2 (2026-08); Pastel 4.0.1 (2026-03) | Mixed; core inputs are quiet, newer scroll and form packages are active | `[DEPS-INK-ECO]` |
| OpenTUI | Zig + TS | 0.7.0 (2026-10-25) | Very active; pre-1.0; powers OpenCode | `[OT-REL]` `[OT-DEPS]` |
| blessed | JS | 0.1.81 (2015-09-03) | Dormant (11 years) | `[BLESSED-DEPS]` |
| neo-blessed | JS | 0.2.0 (2018-06-13) | Dormant | `[NEOB-DEPS]` |
| blessed-contrib | JS | 4.11.0 (2022-02-17) | Dormant | `[BC-DEPS]` |
| terminal-kit | JS | 3.1.4 (2026-07-19) | Active but slow (3.1.2 was 2025-01) | `[TK-DEPS]` |
| @clack/prompts | TS | 1.8.1 (2026-09-13) | Active | `[CLACK-DEPS]` `[CLACK-REL]` |
| Enquirer | JS | 2.4.1 (2023-07-28) | Quiet to dormant; styling docs still marked to-do | `[ENQ-DEPS]` `[ENQ-GH]` |
| Inquirer / @inquirer/prompts | TS | inquirer 14.2.5, @inquirer/prompts 8.7.5 (2026-10-27) | Very active; legacy `inquirer` package described as "maintained, not actively developed" | `[INQ-REL]` `[INQ-GH]` |
| Melker | TS/Deno | CalVer; tag `v2026.03.1` mentioned | Single maintainer, 24 stars, young | `[MELKER]` |
| FTXUI | C++ | v7.0.3 (2026-08-06); docs site shows 7.1.0 (date unverified) | Active | `[FTXUI-REL]` `[FTXUI-COMP]` |
| notcurses | C | release not shown | Large (8.6k commits), 306 open issues; recency unverified | `[NC-GH]` |
| ncurses (form, menu, panel) | C | 6.6 (2025-12-30) | Active, conservative | `[NCURSES-ANN]` |
| tvision | C++ | no stable releases | README: "stick to the latest commit" | `[TVISION-GH]` |
| Brick | Haskell | 3.0 (2026-09-26) | Active; single maintainer | `[BRICK-HK]` `[BRICK-GH]` |
| libvaxis | Zig | v0.5.1 (12 Nov, year not shown; probably 2025); README targets Zig 0.17.0 | README targets Zig 0.17.0, which suggests development has moved past the last tag (inference) | `[VAXIS-REL]` `[VAXIS-GH]` |
| Ratatouille | Elixir | 0.5.1 (2020-03-25) | Dormant | `[RATA-HEX]` `[RATA-GH]` |
| Raxol | Elixir | ~> 2.7 | Active (3,134 commits; benchmarks dated 2026-08) but 81 stars | `[RAXOL-GH]` |
| Terminal.Gui v2 | C# | 2.5.0 (2026-09-11); 2.5.1-develop builds 2026-10-25 | Active; 2.3M downloads | `[TG-NUGET]` `[TG-GH]` |
| Spectre.Console | C# | 0.57.2 (2026-07-02); 0.57.3-alpha builds to 2026-10-05 | Active; 66M downloads; .NET Foundation | `[SPECTRE-NUGET]` `[SPECTRE-GH]` |
| Hex1b | C# | no release shown | Young; 803 commits, 180 stars | `[HEX1B-GH]` |
| Lanterna | Java | 3.2.0-alpha1 (metadata updated 2026-03-15); 3.1.5 newest stable | Quiet; LGPL | `[LANT-MVN]` `[LANT-GH]` |
| Jexer | Java | 13 tags, none identified | The GitLab repo is marked archived | `[JEXER-GL]` |
| Mosaic | Kotlin | 0.18.0 stable; 0.19.0-SNAPSHOT | "Experimental"; Compose runtime | `[MOSAIC-GH]` |
| Kotter | Kotlin | 1.4.0 | Looks maintained; 2 open issues | `[KOTTER-GH]` |
| nocterm | Dart | 0.9.0 (about 2026-08, "two months ago") | Young; 410 stars | `[NOCT-PUB]` `[NOCT-GH]` |

Named in `[AWESOME]` but **not examined**: Ashen (Swift), TUI4J, casciian, Consolonia,
SharpConsoleUI, imtui, FINAL CUT, tuibox, TermDOM, nimwave (Nim), php-tui, moulti, tcell,
termbox2.

## 2. Control-type catalog

Columns: control, providers, API shape, maturity, notes. "Maturity" is a judgement from
the version and maintenance evidence in section 1 plus each page's own caveats.

### 2.1 Text entry and editing

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Text input | Bubbles `textinput`; Huh `Input`; tview `InputField`; @inkjs/ui `TextInput`, `EmailInput`, `PasswordInput`; OpenTUI `Input`; FTXUI `Input`; Terminal.Gui `TextField` and `TextValidateField`; Brick `Edit`; libvaxis `TextInput`; Kotter `input()`; terminal-kit `inputField`; Clack `text`, `password`; Inquirer `input`, `password`, `number` | Bubbles: model with `Update`/`View`, `EchoMode` (normal, password, none), `Validate func(string) error` stored in `Err`, `CharLimit`, `Placeholder`. Terminal.Gui: masked input checked by a validation provider. Kotter: `viewMap` masks, `customFormat` styles per character | High in Charm, Ink, Clack, Inquirer, Terminal.Gui | Bubbles lets you pick a virtual cursor (animated cell) or the real terminal cursor with a shape; real-cursor mode makes the caller add screen offsets. Inquirer 8.7.5 fixed the cursor overlapping the message on an empty password prompt. `[BBL-TI]` `[HUH-PKG]` `[TVIEW-PKG]` `[INKUI-GH]` `[OT-COMP]` `[FTXUI-COMP]` `[TG-VIEWS]` `[BRICK-GH]` `[VAXIS-VXFW]` `[KOTTER-GH]` `[TK-GH]` `[CLACK-README]` `[INQ-GH]` `[INQ-REL]` |
| Autocomplete | Bubbles `textinput` suggestions; Huh `Input.Suggestions` and `SuggestionsFunc`; tview `InputField` autocomplete callbacks and styles; Clack `autocomplete` and `path`; Inquirer `search`; Enquirer `AutoComplete`; Kotter `Completions`; @inkjs/ui `TextInput`; Melker `autocomplete` | Bubbles: `SetSuggestions`, `AcceptSuggestion/NextSuggestion/PrevSuggestion` bindings. Huh: suggestion function recomputed only when a bound pointer changes. Clack: `autocomplete` plus `maxItems` | High | Two shapes exist: inline ghost-text completion (Bubbles, Huh) and a filterable popup list (tview, Clack, Inquirer). `[BBL-TI]` `[HUH-PKG]` `[TVIEW-PKG]` `[CLACK-README]` `[INQ-GH]` `[ENQ-GH]` `[KOTTER-GH]` `[INKUI-GH]` `[MELKER]` |
| Multiline editor | Bubbles `textarea`; tview `TextArea`; Terminal.Gui `TextView`; OpenTUI `Textarea`; Huh `Text`; tvision `TEditor`; Brick `Edit`; Inquirer `editor`; Clack `multiline`; Kotter `multilineInput()` | Bubbles: `ShowLineNumbers`, `DynamicHeight` between `MinHeight` and `MaxHeight`, `MaxContentHeight` counting soft wraps, prompt function per line, drag selection added in v2.2.0, `CopySelection` returns a command, no documented undo. tview: Emacs-style keys, Shift selection, Ctrl-Z/Ctrl-Y undo/redo, UAX #29 word boundaries, double-click word select, private clipboard unless `SetClipboard` is set | High (tview, Terminal.Gui, tvision); Bubbles textarea is newer | Huh `Text` and Inquirer `editor` both hand off to `$EDITOR`. tvision's editor handles files over 64 KiB, preserves LF/CRLF, has a context menu. `[BBL-TA]` `[TVIEW-PKG]` `[TG-GH]` `[OT-COMP]` `[HUH-PKG]` `[TVISION-GH]` `[INQ-GH]` `[CLACK-README]` `[KOTTER-GH]` |
| Undo and redo | tview `TextArea`; Terminal.Gui `TextView` (README lists undo/redo); OpenTUI edit-buffer command `input.undo`; Bubbles `textarea` has none documented | Command-style (`input.undo`) in OpenTUI; fixed keys in tview | Medium | Rare outside full editors. `[TVIEW-PKG]` `[TG-GH]` `[OT-ADDONS]` `[BBL-TA]` |
| Clipboard | Bubble Tea v2 (OSC 52 commands); Bubbles `Paste()` and `CopySelection`; OpenTUI clipboard API; libvaxis OSC 52; tvision `TClipboard` (system, internal fallback, OSC 52, far2l); tview `TextArea` | Terminal escape sequences, so it works over SSH; tview keeps a private clipboard by default and copies on Ctrl-Q because Ctrl-C is quit | Medium | Linux and BSD still need helper tools in Crush's case. `[BT-PKG]` `[BBL-TA]` `[OT-DOCS]` `[VAXIS-GH]` `[TVISION-GH]` `[TVIEW-PKG]` `[CRUSH-GH]` |

### 2.2 Choice, toggle and range

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Checkbox, radio, toggle | Terminal.Gui `CheckBox` (two or three states), `OptionSelector`, `FlagSelector<T>`; FTXUI `Checkbox`, `Radiobox`, `Toggle`; tview `Checkbox`; blessed `Checkbox`, `RadioSet`; Brick form fields `checkboxField`, `radioField`; Huh `Confirm`; @inkjs/ui `ConfirmInput` | FTXUI binds to a `bool*` or `int*`. Terminal.Gui's selectors take enums and `[Flags]` enums | High | OpenTUI ships no checkbox in its component list. Ink exposes `aria-role` values `checkbox`, `radio`, `radiogroup`. `[TG-VIEWS]` `[FTXUI-COMP]` `[TVIEW-PKG]` `[BLESSED-GH]` `[BRICK-GUIDE]` `[HUH-PKG]` `[INKUI-GH]` `[OT-COMP]` `[INK-README]` |
| Select, dropdown, combobox | Huh `Select[T]`, `MultiSelect[T]`; tview `DropDown`, `List`; @inkjs/ui `Select`, `MultiSelect`; FTXUI `Dropdown`, `Menu`; OpenTUI `Select`, `TabSelect`; Terminal.Gui `DropDownList`; Spectre `SelectionPrompt`, `MultiSelectionPrompt`; Clack `select`, `selectKey`, `multiselect`, `groupMultiselect`; Inquirer `select`, `checkbox`, `rawlist`, `expand`; Enquirer `Select`, `MultiSelect`, `Sort`, `Scale`, `Toggle`; Melker `combobox`, `select`; gum `choose`, `filter` | Huh: generic over the value type with `NewOption(label, value)`. OpenTUI `Select`: options carry `name`, `description`, `value`; `wrapSelection` off by default; events `ITEM_SELECTED` and `SELECTION_CHANGED`. Spectre: `PageSize` (10), `EnableSearch`, `AddChoiceGroup`, `UseConverter`, `WrapAround`, `DisabledStyle`, `SelectionMode` `Leaf` or `Independent` | High | Spectre warns its prompts are not thread safe and must not run alongside progress displays. Clack `selectKey` picks by a one-character value. `[HUH-PKG]` `[TVIEW-PKG]` `[INKUI-GH]` `[FTXUI-COMP]` `[OT-SELECT]` `[TG-VIEWS]` `[SPECTRE-SEL]` `[SPECTRE-MSEL]` `[CLACK-README]` `[INQ-GH]` `[ENQ-GH]` `[MELKER]` `[GUM-GH]` |
| Date and time picker | Terminal.Gui `DatePicker` (calendar), `DateEditor`, `TimeEditor` (culture-aware); Clack `date` (year, month, day with `minDate`, `maxDate`) | Terminal.Gui: view classes. Clack: single async prompt | Medium; very few providers | Enquirer lists `DatePrompt` as "Coming Soon". Spectre's `Calendar` is display only. I found nothing in Bubbles, Huh, Ink, FTXUI, tview or Brick. `[TG-VIEWS]` `[CLACK-README]` `[ENQ-GH]` `[SPECTRE-SITE]` |
| Slider and range | Terminal.Gui `LinearSelector`, `LinearMultiSelector`, `LinearRange`, `NumericUpDown`; FTXUI `Slider`; OpenTUI `Slider` (draggable thumb); Enquirer `Scale`; Melker `slider` | FTXUI: `Slider(SliderOption<T>)` with int/float/long shorthands. Terminal.Gui range type is typed by the option list | Medium | Absent from Bubbles, Huh, Ink, tview. `[TG-VIEWS]` `[FTXUI-COMP]` `[FTXUI-HPP]` `[OT-COMP]` `[ENQ-GH]` `[MELKER]` |

### 2.3 Collections: lists, tables, trees

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Virtual list with pagination and fuzzy filter | Bubbles `list`; Brick `List`; tview `List`; Terminal.Gui `ListView<T>`; libvaxis `ListView`; ink-virtual-list and ink-scroll-list; gum `filter`; Spectre `SelectionPrompt` search | Bubbles: `Item` needs only `FilterValue() string`; an `ItemDelegate` supplies `Render`, `Height`, `Spacing`, `Update`, and may implement `help.KeyMap` so its keys join the help bar. `FilterState` is `Unfiltered`, `Filtering`, `FilterApplied`. Fuzzy match is `sahilm/fuzzy`; `MatchesForItem(i)` returns matched rune indexes for highlighting. Pagination is delegated to an embedded `paginator.Model`. The list also embeds a spinner, status bar, help, and a transient status message (default 1 s) | High | Brick scrolls through `visible` requests so only on-screen items render, and added optional wrap-around selection in 2.13. The list `FilterFunc` is swappable. `InfiniteScrolling` exists but its behaviour is undocumented. `[BBL-LIST]` `[BBL-PAG]` `[BRICK-GUIDE]` `[BRICK-CL]` `[TVIEW-PKG]` `[TG-VIEWS]` `[VAXIS-VXFW]` `[INK-README]` `[GUM-GH]` `[SPECTRE-SEL]` |
| Table (interactive) | Bubbles `table`; Evertras bubble-table; stickers `Table`; tview `Table`; Terminal.Gui `TableView`; FTXUI `Table`; OpenTUI `TextTable`; blessed `ListTable`; ink-table; libvaxis `Table` | Bubbles: `Column{Title, Width}`, rows are `[]string`, three styles (`Header`, `Cell`, `Selected`), no sort or filter. Evertras: multi-column sort with natural ordering, a `/` filter footer, page size or target height, frozen columns, horizontal scroll, style functions. stickers: flexbox-based, sort and filter ticked in its TODO. tview: `SetSelectable`, `SetFixed` for header rows and columns, `TableContent` interface for virtual data, no sort | Medium; interactive tables with sort/filter are community-built | tview's `TableContent` lets rows live outside memory and warns that `SetEvaluateAllRows(true)` can be slow. `[BBL-TBL]` `[EVT-GH]` `[STICK-GH]` `[TVIEW-PKG]` `[TG-VIEWS]` `[FTXUI-DOM]` `[OT-COMP]` `[BLESSED-GH]` `[INK-README]` `[VAXIS-W]` |
| Table (static rendering) | Lip Gloss `table`; Brick `Table`; Spectre `Table`; pterm `Table`; Ratatouille `table` | Lip Gloss: `Data` interface (`At`, `Rows`, `Columns`), `StyleFunc(row, col)`, `HeaderRow` sentinel (-1), `Wrap(bool)` (off truncates with an ellipsis), per-part border toggles, `YOffset` plus `VisibleRows()` for scrolling, `NewFilter(data)` returning a `Data`. Brick: cells must be `Fixed`, rows equal length, per-row and per-column alignment, borders join automatically | High | Brick's table has no selection. Spectre has 20+ border styles. `[LG-TABLE]` `[BRICK-TABLE]` `[SPECTRE-SITE]` `[PTERM-GH]` `[RATA-GH]` |
| Tree view | Bubbles `tree` (new); Lip Gloss `tree` (static); tview `TreeView`; Terminal.Gui `TreeView<T>`; blessed-contrib `tree`; Ratatouille `tree`; Spectre `Tree` (static); tvision `TOutlineViewer` | Bubbles tree: `Node` with `Open/Close/IsOpen`, `Hidden`, per-node enumerator and indenter styles, open/closed/cursor characters, `ScrollOff`, half-page and top/bottom navigation, `ShortHelp/FullHelp`. Built on Lip Gloss tree primitives | Bubbles tree: new, 0 known importers on pkg.go.dev. tview, Terminal.Gui: mature | Lip Gloss enumerators include Arabic, Alphabet, Roman, Bullet, and Tree. `[BBL-TREE]` `[LG-PKG]` `[TVIEW-PKG]` `[TG-VIEWS]` `[BC-GH]` `[RATA-GH]` `[SPECTRE-SITE]` `[TVISION-GH]` |
| Pagination | Bubbles `paginator` (Dots or Arabic, `GetSliceBounds`); Evertras `WithPageSize` or `WithTargetHeight` | Pure page math plus an indicator; it does not draw content | High | Zero-indexed pages. `[BBL-PAG]` `[EVT-GH]` |

### 2.4 Navigation and containers

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Tabs | FTXUI `Container::Tab` with `Toggle` or `Menu`; Terminal.Gui `Tabs`; OpenTUI `TabSelect`; ink-tab; blessed `Listbar`; Melker `tabs` | FTXUI: `Tab(children, int* selector)`; only the selected child is drawn and interactive. OpenTUI: horizontal strip of choices | High | tview `Pages` switches pages but has no tab bar. Brick has no tab widget in its module list. `[FTXUI-COMP]` `[TG-VIEWS]` `[OT-COMP]` `[INK-README]` `[BLESSED-GH]` `[MELKER]` `[TVIEW-PKG]` `[BRICK-HK]` |
| Accordion, collapsible | FTXUI `Collapsible(label, child, Ref<bool>)`; moulti (shell tool with collapsible output blocks) | One toggled child | Low; few providers | Bubbles `tree` nodes open and close, but there is no standalone accordion. `[FTXUI-HPP]` `[AWESOME]` `[BBL-TREE]` |
| Split pane | FTXUI `ResizableSplitLeft/Right/Top/Bottom` (mouse resizable); libvaxis vxfw `SplitView`; Hex1b splitter | FTXUI binds the main size to an `int*` | Medium | Lip Gloss, Spectre `Layout`, and tview `Flex`/`Grid` lay out fixed or ratio regions but are not user-resizable. `[FTXUI-COMP]` `[VAXIS-VXFW]` `[HEX1B-GH]` `[LG-PKG]` `[SPECTRE-LAYOUT]` `[TVIEW-PKG]` |
| Layout engine | Flexbox: Ink and OpenTUI (Yoga), FTXUI, tview `Flex`, stickers, Melker, vxfw `FlexRow/FlexColumn`, Hex1b (`Fill`, `Content`, `Fixed`). Grid: tview, blessed-contrib, Kotter, FTXUI `gridbox`. Constraint solver: Ultraviolet `layout` (Cassowary with length, min, max, percent, ratio, fill). Terminal.Gui `Pos`/`Dim` | Declarative nesting in most; Ultraviolet exposes a solver as a standalone package | High | Brick uses `Fixed`/`Greedy` growth policies and `hLimit`/`vLimit`; scrollable directions must be `Fixed` or it throws. `[INK-GH]` `[OT-DOCS]` `[FTXUI-DOM]` `[TVIEW-PKG]` `[STICK-GH]` `[MELKER]` `[HEX1B-GH]` `[BC-GH]` `[KOTTER-GH]` `[UV-GH]` `[TG-GH]` `[BRICK-GUIDE]` |
| Modal and dialog | Terminal.Gui `Dialog`, `Dialog<T>`, `Prompt<T>`, `Wizard`; tview `Modal`; Brick `Dialog`; FTXUI `Modal`; blessed `Prompt`, `Question`, `Message`; Melker `dialog`, `alert`, `confirm`, `prompt`; Lanterna ("basically all windows are modal") | Terminal.Gui `Prompt<T>` wraps any view with Ok and Cancel and extracts a typed result. FTXUI `Modal(main, modal, bool*)` | High | Bubble Tea ships no dialog; Crush builds its own. `[TG-VIEWS]` `[TVIEW-PKG]` `[BRICK-HK]` `[FTXUI-COMP]` `[BLESSED-GH]` `[MELKER]` `[LANT-GH]` `[CRUSH-GH]` |
| Popup, popover, tooltip | Terminal.Gui `PopoverMenu`; Lip Gloss `Layer`/`Compositor` with z-order; FTXUI `dbox` and `clear_under`; ncurses panels (stacking, show, hide, `update_panels`); notcurses planes | Layer primitives, not ready-made widgets | Medium | I found no tooltip component in any surveyed project. `[TG-VIEWS]` `[LG-PKG]` `[FTXUI-DOM]` `[NCURSES-PANEL]` `[NC-GH]` |
| Menu bar and context menu | Terminal.Gui `MenuBar`, `MenuItem`, `PopoverMenu`; Brick `Menu` and `MenuBar` (3.0); tvision `TMenuBar`, context menu; blessed `Listbar` | Brick: three flavours (`SimpleMenuBar`, `DispatchingMenuBar`, general); `handleMenuBarEvent` returns `True` if it consumed the event; entries can display their bound key | High (Terminal.Gui, tvision); new in Brick | Brick menu items show the first known binding of an abstract event. `[TG-VIEWS]` `[BRICK-MB]` `[BRICK-MENU]` `[TVISION-GH]` `[BLESSED-GH]` |
| Breadcrumb | None found in any surveyed project | n/a | n/a | Treat as a gap that nobody fills. |
| Focus management | Brick `FocusRing` (circular, by resource name, `withFocusRing`, `focusRingCursor`); Ink `useFocus`, `useFocusManager`; Terminal.Gui; tview `SetFocus`, `Focus/Blur` on every primitive; OpenTUI single focused renderable, no Tab traversal in core; FTXUI containers navigate with arrows or j/k and h/l; vxfw bubbles key events up the tree | Brick keeps focus by name, not widget; OpenTUI auto-focuses the nearest focusable on left press unless `autoFocus: false` | High | Terminal.Gui's tenet "If It's Hot, It Works" fixed a v1 bug where menu hotkeys stayed live beneath modals. `[BRICK-FOCUS]` `[INK-GH]` `[TG-KB]` `[TVIEW-PKG]` `[OT-INTERACT]` `[FTXUI-COMP]` `[VAXIS-GH]` |
| Mouse zones and hit testing | bubblezone (zero-width markers, `Mark`, `Scan`, `InBounds`, `Pos`); Lip Gloss v2 `Compositor.Hit(x, y)` returning the top layer with an ID; Brick `clickable` and extents; OpenTUI (hit = last painted node, bubbling, `preventDefault`); FTXUI `Hoverable`; Bubble Tea v2 `View.OnMouse` interceptor | bubblezone only works in alt screen and breaks if output is hard-trimmed with `MaxHeight`/`MaxWidth` | Medium | OpenTUI does not synthesise a click event: a click is a down then an up. `[BZ-GH]` `[LG-PKG]` `[BRICK-GUIDE]` `[OT-INTERACT]` `[FTXUI-COMP]` `[BT-UP]` |

### 2.5 Content display

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Pager and viewport | Bubbles `viewport`; gum `pager`; tview `TextView`; Brick viewports; OpenTUI `ScrollBox`, `ScrollBar`; FTXUI `frame`, `vscroll_indicator`; ink-scroll-view; Terminal.Gui `ScrollBar`, `ScrollSlider`; libvaxis `ScrollView` | Bubbles: `SoftWrap`, horizontal offset and step, `LeftGutterFunc` (fixed width, receives index, total lines, soft-wrap flag), `SetHighlights` ranges with focused-match navigation, `StyleLineFunc`, `FillHeight`, mouse wheel delta (default 3) | High | Viewport highlights plus gutter is the closest thing to a built-in search-in-pager. Brick's `visible` request scrolls only when the item is off screen. `[BBL-VP]` `[GUM-GH]` `[TVIEW-PKG]` `[BRICK-GUIDE]` `[OT-COMP]` `[FTXUI-DOM]` `[INK-README]` `[TG-VIEWS]` `[VAXIS-W]` |
| Log viewer | blessed `Log`; blessed-contrib `log`; tview `TextView` (safe to write from any goroutine; colour tags; regions and highlights); Ink `<Static>`; Kotter `aside`; Bubble Tea `Println` | Append-only region. Ink `<Static>` and Bubble Tea `Println` print above the live UI, which keeps finished output in scrollback | High | A distinct pattern from a viewport: finished lines leave the redraw region. `[BLESSED-GH]` `[BC-GH]` `[TVIEW-PKG]` `[INK-README]` `[KOTTER-GH]` `[BT-PKG]` |
| Markdown and rich text | Glamour; OpenTUI `Markdown` (React/Solid inline nodes including `<a>`); Terminal.Gui `Markdown`, `MarkdownCodeBlock` (copy button), `MarkdownTable`; ink-markdown; gum `format`; blessed-contrib `markdown`; tview colour tags | Glamour: `TermRenderer`, JSON or built-in stylesheets, `WithWordWrap` (default 80), `WithTableWrap`, `WithBaseURL`, streaming `Write`/`Close`/`Read` | High | Glamour is deliberately pure: it does not downsample colours, so pair it with Lip Gloss. `[GLAM-PKG]` `[OT-COMP]` `[TG-VIEWS]` `[INK-README]` `[GUM-GH]` `[BC-GH]` `[TVIEW-PKG]` |
| Syntax highlighting | Glamour (chroma formatter option); OpenTUI `Code` (Tree-sitter; bundled JS/TS, Markdown, Zig parsers); Terminal.Gui `Code`; ink-syntax-highlight; Brick via the separate `brick-skylighting` package | Tree-sitter or chroma | High | OpenTUI needs Tree-sitter configuration for other grammars. `[GLAM-PKG]` `[OT-COMP]` `[TG-VIEWS]` `[INK-README]` `[BRICK-GH]` |
| Diff viewer | OpenTUI `Diff` (unified or split, one file's patch) | Renderable with line-number gutter support | Low; one provider | No other surveyed project ships one. `[OT-COMP]` |
| Progress bar and spinner | Bubbles `progress` (half-block fill, blend gradient, `WithScaled`, spring animation, `ViewAs` for static), `spinner` (12 built-ins, custom `Frames` and `FPS`); Huh `spinner`; @inkjs/ui `Spinner`, `ProgressBar`; FTXUI `gauge`, `spinner`; Terminal.Gui `ProgressBar`, `SpinnerView`; Spectre `Progress`, `Status` (80+ spinners); Clack `spinner`, `progress`, `tasks`, `taskLog`; Brick `ProgressBar`; terminal-kit `progressBar`; pterm; Kotter (built from `textAnimOf` and live vars) | Bubbles: a `Tick` message with a per-spinner ID so multiple spinners coexist; progress animates via `FrameMsg` | High | Clack `taskLog` shows live subprocess output and clears it on success, which is a useful pattern for agent runs. `[BBL-PROG]` `[BBL-SPIN]` `[HUH-PKG]` `[INKUI-GH]` `[FTXUI-DOM]` `[TG-VIEWS]` `[SPECTRE-SITE]` `[CLACK-README]` `[BRICK-HK]` `[TK-GH]` `[PTERM-GH]` `[KOTTER-GH]` |
| Charts and sparklines | ntcharts (sparkline, bar, line, time series, scatter, heatmap, picture); blessed-contrib (line, bar, stacked bar, map, gauge, donut, sparkline, LCD); Terminal.Gui `GraphView`; Spectre `BarChart`, `BreakdownChart`; FTXUI `Canvas`; ink-chart; pterm; Ratatouille `chart`, `sparkline`, `bar` | ntcharts: a character canvas styled with Lip Gloss, mouse via bubblezone | Medium to high | ntcharts' `v2` number tracks Bubble Tea compatibility, not its own API stability. `[NTC-GH]` `[BC-GH]` `[TG-VIEWS]` `[SPECTRE-SITE]` `[FTXUI-DOM]` `[INK-README]` `[PTERM-GH]` `[RATA-GH]` |
| Badge, alert, status message | @inkjs/ui `Badge`, `StatusMessage`, `Alert` (success, error, warning, info); `UnorderedList`/`OrderedList` | Props with variants | Medium | Static, not timed toasts. `[INKUI-GH]` |
| File picker | Bubbles `filepicker`; Huh `FilePicker`; gum `file`; Brick `FileBrowser` (multi-select, 2.11 fix); Terminal.Gui `FileDialog`, `OpenDialog`, `SaveDialog`; blessed `FileManager`; Clack `path`; terminal-kit `fileInput`; tvision file dialogs | Bubbles: `AllowedTypes`, `FileAllowed/DirAllowed`, `ShowHidden/ShowPermissions/ShowSize`, helpers `DidSelectFile` and `DidSelectDisabledFile` read the message instead of defining new ones | High | Bubbles and Huh share keys: open, back, select, plus page and top/bottom jumps. `[BBL-FP]` `[HUH-PKG]` `[GUM-GH]` `[BRICK-CL]` `[TG-VIEWS]` `[BLESSED-GH]` `[CLACK-README]` `[TK-GH]` `[TVISION-GH]` |

### 2.6 Commands, hints and notifications

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Key-hint and help bar | Bubbles `key` + `help`; Huh (`Form.KeyBinds()`, `WithShowHelp`); Brick `Keybindings.Pretty`; Terminal.Gui `Shortcut` + `StatusBar`; OpenTUI keymap metadata (`desc`, `group`); Inquirer `theme.style.help` and `key` | See section 3.1 | High | The strongest design-system idea in this survey. `[BBL-KEY]` `[BBL-HELP]` `[HUH-PKG]` `[BRICK-KB-PRETTY]` `[TG-KB]` `[OT-ADDONS]` `[INQ-CORE]` |
| Command palette | Melker `command-palette` element; OpenTUI keymap named commands plus Ex-style `:name args` input; Crush's own `ctrl+p` palette (app code); gum `filter` | Melker: markup element. OpenTUI: commands and queries (`getCommands`, `getActiveKeys`) a palette can render | Low as a shipped widget | A palette is mostly app code over a named-command registry. `[MELKER]` `[OT-KEYMAP]` `[OT-ADDONS]` `[CRUSH-GH]` `[GUM-GH]` |
| In-app toast | None found as a component. Closest: Bubbles list status message (default 1 s lifetime) | Timed string set through a command | n/a | Terminal-level desktop notifications exist: OpenTUI `triggerNotification` (OSC 99, 777, 9), libvaxis (OSC 9, 777), Crush modes `auto`, `native`, `osc`, `bell`, `disabled`. `[BBL-LIST]` `[OT-NOTIF]` `[VAXIS-GH]` `[CRUSH-GH]` |
| Program status | Bubble Tea v2.1.0 (OSC 7501); OpenTUI `setProgramStatus`; libvaxis (OSC 7501) | States `idle`, `working`, `blocked` (kind `permission`, `question`, `auth`), `done`, `error`, `clear`; optional `progress` 0 to 100; slash-separated `id` for multiple tasks | New (2026); spec at superlogical.com | See section 5; it targets exactly the "what is each agent doing" question. `[BT-REL]` `[OT-PS]` `[VAXIS-GH]` |
| Animation | Harmonica (spring, projectile); Bubbles `progress` spring; Brick `Animation` (clips of frames, `Loop`/`Once`, `pingPongClip`); FTXUI `RequestAnimationFrame`; OpenTUI timeline; Ink `useAnimation`; Mosaic `mosaic-animation`; Kotter `textAnimOf` | Harmonica: `NewSpring(dt, angularFreq, damping)` with `Update(pos, vel, target)`. Brick: manager thread ticks and posts state updates through a channel | Medium | Brick states animation is frames on a timer, with no easing. FTXUI 7.0.2 reverted to producing frames only on request after a 60 fps redraw loop. `[HARM-GH]` `[BBL-PROG]` `[BRICK-ANIM]` `[FTXUI-REL]` `[OT-DOCS]` `[INK-GH]` `[MOSAIC-GH]` `[KOTTER-GH]` |

### 2.7 Forms, themes, accessibility, testing

| Control | Providers | API shape | Maturity | Notes |
| --- | --- | --- | --- | --- |
| Form, groups, wizard, validation | Huh (`Form` > `Group` > field); Brick `Forms` (typed, lens-bound); tview `Form`; ncurses `form` (fields, pages, custom field types); Terminal.Gui `Wizard`, `WizardStep`; Clack `group`; Enquirer `Form`, `Survey`, `Snippet`; ink-form; ink-stepper | See section 3.3 | High | Huh `Form` is itself a Bubble Tea model. `[HUH-PKG]` `[BRICK-GUIDE]` `[TVIEW-PKG]` `[NCURSES-FORM]` `[TG-VIEWS]` `[CLACK-README]` `[ENQ-GH]` `[INK-README]` |
| Theme and design tokens | Huh; Bubbles; Lip Gloss; Terminal.Gui; Brick; @inkjs/ui; Inquirer; Glamour; Crush; tview; tvision | See section 3.2 | High | |
| Accessible or non-interactive fallback | Huh `WithAccessible`; Ink screen-reader mode with ARIA props | See section 3.4 | Low; two providers | |
| Terminal capability handling | colorprofile (`TrueColor`, `ANSI256`, `ANSI`, `Ascii`, `NoTTY`); FTXUI (`NO_COLOR`, `SetColorSupport`); Spectre (3, 4, 8, 24 bit with downgrade); libvaxis (query-based, no terminfo); Ultraviolet (no terminfo) | `colorprofile.NewWriter` rewrites ANSI to fit the detected profile | High | Lip Gloss v2 `Complete(profile)` picks ANSI, ANSI256, or TrueColor variants explicitly. `[CPROF]` `[FTXUI-REL]` `[SPECTRE-GH]` `[VAXIS-GH]` `[UV-GH]` `[LG-PKG]` |
| Unicode and width | Ultraviolet width method on every screen; notcurses extended grapheme clusters and `wcswidth` matching; libvaxis Unicode core (mode 2027) and explicit width; OpenTUI bidi marks occupy zero cells (0.6.7); Bubble Tea 2.0.8 emoji fixes; tview uses `uniseg`; tvision UTF-8 throughout; Kotter `textMetrics`; terminal-kit has a combining-character doc | Each library owns a width function | Mixed | Width bugs are a recurring release-note theme. `[UV-GH]` `[NC-GH]` `[VAXIS-GH]` `[OT-REL]` `[BT-REL]` `[TVIEW-GH]` `[TVISION-GH]` `[KOTTER-GH]` `[TK-GH]` |

## 3. Design-system ideas worth borrowing

### 3.1 Key-binding objects that generate help

Four independent designs reached the same idea (one definition drives dispatch and help)
with different trade-offs:

1. **Bubbles `key.Binding` plus `help.KeyMap`** `[BBL-KEY]` `[BBL-HELP]`
   - A `Binding` holds key strings, a `Help{Key, Desc}` pair, and an enabled flag. A
     disabled binding is skipped by matching and by help. `Unbind()` is the stronger
     reset.
   - `help.KeyMap` has only `ShortHelp() []Binding` and `FullHelp() [][]Binding`
     (each inner slice is a column). Components such as `list`, `table`, `tree`
     implement it, and the list embeds delegates' bindings.
   - `help.Model` truncates the short view to a width with an ellipsis. Styles come as
     `ShortKey/ShortDesc/FullKey/...` with `DefaultStyles(isDark)`.
   - Apps own the keymap struct. The package ships only the building blocks.
2. **Brick abstract events** `[BRICK-KD]` `[BRICK-KB-PRETTY]` `[BRICK-GUIDE]`
   - Handlers are created with `onEvent <abstractEvent> "description" action`. Each
     event has a unique readable name. Default bindings live in a `KeyConfig`; users
     override them from an INI file.
   - `keyDispatcher` returns `Left` with the list of colliding bindings when two
     handlers share a key, so collisions fail at startup, not at runtime. Modal
     dispatchers avoid false collisions.
   - `Brick.Keybindings.Pretty` renders the same handlers as a plain-text table, a
     Markdown table, or an embeddable widget, grouped by caller-supplied sections.
     Menu entries can show the first known binding of their event.
3. **Terminal.Gui commands and layered bindings** `[TG-KB]`
   - A `Command` is an operation a view declares support for. Bindings map keys to
     commands in three layers: application defaults, view-type defaults, per-view
     defaults. User overrides come from JSON keyed by view type name, and each layer
     skips keys an earlier layer already bound.
   - `PlatformKeyBinding` carries per-OS keys.
   - `Shortcut` pairs command, help text, and key, and feeds `StatusBar`, `MenuBar`,
     and `PopoverMenu` so all three agree.
   - Ordered tenets, "Users Have Control" first, settle conflicts.
4. **OpenTUI keymap** `[OT-KEYMAP]` `[OT-ADDONS]`
   - Layers (global or attached to a focus target) carry commands and bindings, with
     `priority`; named commands (`file.save`) are separate from keys.
   - Metadata addon compiles `desc`, `group`, `category`, `title` onto bindings and
     commands. `getActiveKeys()` and `getPendingSequence()` let a hint overlay show
     only keys reachable after a half-typed sequence.
   - Optional addons add leader keys with timeout, Emacs-style chords, `mod+` as the
     host's preferred modifier, dead-binding warnings, and layout fallback so Cyrillic
     `Ctrl+с` still matches `ctrl+c`.
   - Core has no built-in help popup; the data model is there, the UI is not.

Also: Inquirer reads `INQUIRER_KEYBINDINGS=vim,emacs` so one environment variable adds
`j/k` and `Ctrl-p/n` navigation to every prompt `[INQ-CORE]`. Huh exposes per-field
keymaps (`InputKeyMap`, `SelectKeyMap`, ...) that can be applied to a form, group, or
field `[HUH-PKG]`.

### 3.2 Adaptive and semantic colour tokens

- **Light and dark as an argument, not a global.** Lip Gloss v2 replaced implicit
  detection with `LightDark(isDark)` returning a picker, and `Complete(profile)` for
  per-profile variants. In Bubble Tea the flag comes from `BackgroundColorMsg.IsDark()`
  `[LG-PKG]`. Bubbles ships `DefaultStyles(isDark)`, `DefaultDarkStyles()`,
  `DefaultLightStyles()` for every component `[BBL-TA]` `[BBL-HELP]`. Huh's whole theme
  type is `Theme(isDark bool) *Styles` `[HUH-PKG]`.
- **Structured style bundle per component.** Huh's `Styles` has `Form`, `Group`
  (`Base`, `Title`, `Description`), `FieldSeparator`, `Focused` and `Blurred`
  `FieldStyles`, plus `Help`. A theme can be set at form, group, or field level
  `[HUH-PKG]`. Bubbles textarea uses `Focused`/`Blurred` `StyleState` plus `Cursor`
  `[BBL-TA]`.
- **Semantic roles with derivation (Terminal.Gui).** A `Scheme` maps `VisualRole`
  (`Normal`, `Focus`, `Disabled`, ...) to an attribute. Only `Normal` is mandatory; the
  rest derive by fixed rules, and the derivation is light/dark aware (brighter means
  more lightness on dark backgrounds, reversed on light). `Color.None` resolves from the
  terminal's own colours via OSC 10/11 before any colour math. A disabled view
  automatically uses the `Disabled` role. A `GettingAttributeForRole` event lets code
  override a lookup `[TG-SCHEME]`.
- **Hierarchical attribute names (Brick).** Names like `list.selected` inherit from
  `list` the way CSS specificity does. A theme stores defaults and user customisations
  separately, so both can be written; customisations load and save as INI, with hex
  colours degraded to 256 colours. `themeDescriptions` holds per-attribute docs so
  customisation docs can be generated `[BRICK-GUIDE]` `[BRICK-THEMES]`.
- **Style functions of state (@inkjs/ui).** Component themes are `{styles, config}`
  where each style entry is a function of props or state, read through
  `useComponentTheme('Name')`, supplied by `ThemeProvider`, and extended with
  `extendTheme(defaultTheme, {...})` `[INKUI-GH]`.
- **Theme object with defaults and merge (Inquirer).** `makeTheme(promptDefaults,
  userTheme)` layers `prefix` (idle and done), `spinner`, `keybindings`, and `style`
  functions (`answer`, `message`, `error`, `help`, `highlight`, `key`) `[INQ-CORE]`.
- **Palette intent (OpenTUI).** Each `RGBA` carries intent `rgb`, `indexed` (ANSI slot),
  or `default` (terminal fg or bg). Blending an indexed colour yields plain RGB
  `[OT-COLORS]`. This separates "use the user's palette" from "use this exact colour".
- **Theme files and live preview (Crush).** Built-in themes plus small JSON palettes
  (`base`, `primary`, `bg_base`); the picker previews each theme as you move, `enter`
  confirms, `esc` reverts; `ctrl+e` edits the highlighted theme live `[CRUSH-GH]`.
- **Stylesheet as data (Glamour).** Markdown styling is JSON (`WithStylesFromJSONFile`)
  or an `ansi.StyleConfig` `[GLAM-PKG]`.
- **Counter-example:** tview has one global `Styles` variable, dark by default, with the
  note that terminals rarely report their scheme so it does not follow it `[TVIEW-PKG]`.

### 3.3 Form-group abstractions

- **Huh** `[HUH-PKG]` `[HUH-README]`
  - `Form` holds `Group`s (pages) that hold fields. Layouts: default (one group at a
    time), `LayoutStack`, `LayoutColumns(n)`, `LayoutGrid(r, c)`.
  - Groups hide with `WithHide` or `WithHideFunc`. Fields expose `Skip()` and `Zoom()`.
    A group with field errors blocks navigation to the next group.
  - Dynamic content: `TitleFunc`, `DescriptionFunc`, `PlaceholderFunc`,
    `OptionsFunc`, `SuggestionsFunc` take a function plus a bound pointer; the function
    re-runs only when the bound value changes and its result is cached.
  - Generic `Select[T]` and `MultiSelect[T]`; `Accessor[T]` decouples fields from
    storage.
  - `Form.State` is `StateNormal`, `StateCompleted`, `StateAborted`. Sentinel errors for
    abort and timeout. Any field can run standalone.
- **Brick Forms** `[BRICK-GUIDE]`: a `Form` is built over the app's state record; each
  field constructor takes a lens into the record, so values are typed with no casts.
  `formState` returns the last valid values; `invalidFields` and `setFieldValid` include
  IO-based checks. Tab and Shift-Tab focus movement is built in. Apps style two
  attributes, focused and invalid.
- **Wizard** `[TG-VIEWS]`: Terminal.Gui `Wizard`/`WizardStep` with help text in the step
  padding, and `Prompt<T>` wrapping any view in Ok/Cancel with a typed result.
- **Sequential prompts** `[CLACK-README]` `[INQ-GH]`: Clack `group` passes earlier
  results to later prompts with one `onCancel`; Inquirer documents conditional questions
  and `ExitPromptError` handling as recipes rather than API.
- **Headless core under a styled layer.** `@clack/core` provides "headless, unstyled
  prompt primitives" and `@clack/prompts` the styled set `[CLACK-GH]`. Inquirer's
  `createPrompt` renders from hooks (`useState`, `useKeypress`, `usePagination`, ...)
  and returns a string or a `[promptLine, below]` tuple `[INQ-CORE]`.

### 3.4 Accessibility and non-interactive fallbacks

- **Huh accessible mode:** `form.WithAccessible(true)` replaces the TUI with plain
  line prompts for screen readers and dictation; the README recommends driving it from
  an environment variable (example `ACCESSIBLE`). Timeouts are not supported in this
  mode `[HUH-README]` `[HUH-PKG]`.
- **Ink screen-reader mode:** `isScreenReaderEnabled` or `INK_SCREEN_READER=true`
  switches rendering to a text form. Props `aria-label`, `aria-hidden`, `aria-role`,
  `aria-state`. Roles include `button`, `checkbox`, `combobox`, `list`, `listbox`,
  `menu`, `menuitem`, `option`, `progressbar`, `radio`, `tab`, `table`, `textbox`,
  `timer`. States include `busy`, `checked`, `disabled`, `expanded`, `selected`. A
  checked checkbox announces as "(checked) checkbox: Accept terms and conditions".
  Ink says it covers only a small portion of ARIA `[INK-README]`.
- **Colour and TTY degradation:** colorprofile strips ANSI for `NoTTY` and quantises
  otherwise `[CPROF]`; Lip Gloss v2 strips colour when output is not a TTY `[LG-PKG]`;
  FTXUI 7.0.1 honours `NO_COLOR` `[FTXUI-REL]`.
- **Keyboard alternatives for mouse:** OpenTUI's docs tell authors to provide keyboard
  alternatives and visible focus states rather than rely on hover or drag
  `[OT-INTERACT]`.
- **Not found:** no accessibility statement in the fetched pages for Terminal.Gui,
  Hex1b, Spectre, Brick, FTXUI, libvaxis, or Clack (absence of evidence only).

### 3.5 Testing approaches

| Approach | Provider | How it works |
| --- | --- | --- |
| Golden files of terminal output | Charm teatest (v2 pseudo-version 2026-10-25) and `x/exp/golden` | `NewTestModel`, `Send`, `Type`, `WaitFor(cond)` polling every 50 ms for up to 1 s by default, `FinalModel`, `RequireEqualOutput` against golden files, `-update` flag. golden's `RequireEqual` escapes control codes before comparing. Needs system `diff`. Experimental. `[TEATEST]` `[TEATEST2]` `[GOLDEN]` |
| Recorded tapes | VHS | `.tape` scripts with `Wait` on regex; `.txt` or `.ascii` output committed as a golden; Hex1b also documents VHS tape parsing and golden-text capture. `[VHS-GH]` `[HEX1B-GH]` |
| Deterministic start state | Bubble Tea v2 | `WithColorProfile(p)` and `WithWindowSize(w, h)` options. `[BT-UP]` |
| In-memory renderer | OpenTUI | `createTestRenderer({width, height})`; frames go to a discarding sink; `mockInput` (`pressKey`, `typeText`, `pasteBracketedText`) and `mockMouse`; `captureCharFrame()` and `captureSpans()`; waits bounded by frame counts, not wall clock; `ManualClock`, `TestRecorder`; separate `@opentui/keymap/testing`. Docs advise testing "the smallest observable boundary that can fail". `[OT-TEST]` |
| Last-frame assertions | ink-testing-library | `render()` then `lastFrame()` string compare. `[INK-README]` |
| Virtual terminal screen | @inquirer/testing | `getScreen()` strips ANSI; `nextRender()` merges rapid renders; `getFullOutput()` runs escapes through a virtual terminal; inline snapshots; `screen` entry points for Vitest and Jest mock all `@inquirer/*` prompts for CLI tests. `[INQ-TEST]` |
| Console injection | Spectre.Console.Testing | Code takes `IAnsiConsole`; tests pass `TestConsole`; `Input.PushTextWithEnter()` queues answers; assert on `Output`. `[SPECTRE-TEST]` |
| Input injection with virtual time | Terminal.Gui | `app.InjectKey`, `IKeyboard` isolates keyboard state so tests run in parallel, `Trace.EnabledCategories` logs key flow. `[TG-KB]` |
| Widget tester | nocterm | `testNocterm`, `pumpComponent`, `sendKey`, `containsText` matcher, modelled on Flutter widget tests. `[NOCT-PUB]` |
| Agent-driven | Raxol | MCP test helper simulates sessions; `mix raxol.rate` runs a render-determinism golden suite. `[RAXOL-GH]` |
| No testing story found | Brick guide has no testing section; libvaxis, FTXUI README silent | |

### 3.6 Architecture and convention ideas

- **Declarative view carries terminal features.** Bubble Tea v2 `View()` returns a
  `tea.View` that declares content plus alt screen, mouse mode, cursor (position, shape,
  colour, blink), focus reporting, window title, progress bar, keyboard enhancements,
  and an `OnMouse` interceptor, replacing imperative commands and program options
  `[BT-UP]`.
- **Delegate pattern for lists.** Bubbles `ItemDelegate` separates item data
  (`FilterValue`) from rendering, height, spacing, and per-item update, and lets a
  delegate contribute keys to help `[BBL-LIST]`.
- **Plugin slots.** OpenTUI hosts declare typed layout regions; plugins contribute
  nodes with an `order`, and a slot mode (`append`, `replace`, `single_winner`) decides
  how plugin output and the host's fallback combine. Errors are buffered and reported
  by phase `[OT-SLOTS]`.
- **Finished output leaves the redraw region.** Ink `<Static>`, Bubble Tea `Println`,
  and Kotter `aside` print permanent lines above the live area `[INK-README]` `[BT-PKG]`
  `[KOTTER-GH]`.
- **Compositing instead of string splicing.** Lip Gloss v2 `Layer`/`Compositor` with Z,
  IDs, and `Hit`; Ultraviolet `Screen` interface shared by terminal, buffers, and
  windows; Canvas implements it `[LG-PKG]` `[UV-GH]`.
- **Hit testing via layer IDs.** bubblezone's marker approach and Lip Gloss `Hit` solve
  the same problem; bubblezone warns it may not work with the v2 compositor
  `[BZ-GH]`.
- **Event bubbling through a widget tree.** vxfw: key events go to the focused widget and
  bubble up the tree; draw receives min/max constraints and a per-frame arena;
  redraw only when a handler asks `[VAXIS-GH]`.
- **Async capability detection with overrides.** OpenTUI notifications: asynchronous
  detection, environment override variables, and an honest "false may just mean not yet
  detected" `[OT-NOTIF]`.
- **Agent-facing components.** Raxol exposes interactive components as MCP tools
  (`click`, `type_into`, `get_value`) with a focus filter narrowing the tool list
  `[RAXOL-GH]`.
- **Document-as-app.** Melker apps are `.melker` or Markdown documents with a declared
  permission policy that can be shown before running `[MELKER]`.
- **CLI chrome.** Fang adds styled help, errors, version, man pages, and completions to
  a Cobra app, with replaceable colour scheme functions `[FANG-PKG]`. gum maps every
  option to a flag and a `GUM_<COMMAND>_<OPTION>` environment variable with flags
  taking precedence (the naming pattern is inferred from examples) `[GUM-GH]`.

## 4. Control-type coverage summary

Present in nearly every full framework surveyed: text input, list or select, table,
progress and spinner, modal or dialog, viewport or scroll, layout containers, mouse
support, key handling.

Present in many: multiline editor, tree, tabs, file picker, forms, markdown, menu bar,
charts, help or key hints, themes.

Present in few (3 or fewer providers): date and time picker, slider, accordion, diff
viewer, command palette, split pane with drag resize, accessible mode, undo, syntax
highlighting with Tree-sitter, tooltips, program status protocol.

Present in none I found: breadcrumb, tooltip, in-app toast component (only desktop
notification escape sequences and static status messages).

## 5. Surprises and items relevant to Conspectus

- **Program Status Protocol (OSC 7501).** Bubble Tea v2.1.0 (2026-10-08) added it, and
  OpenTUI and libvaxis implement it. A program tells its terminal `idle`, `working`,
  `blocked` (`permission`, `question`, `auth`), `done`, or `error`, with optional
  progress, a task `id` path, and a title and message. The Bubble Tea release page
  summary I read says it is aimed mainly at AI agents. OpenTUI's page does not mention
  agents and warns multiplexers need to implement it themselves. This overlaps with Conspectus's
  agent-state survey domain. `[BT-REL]` `[OT-PS]` `[VAXIS-GH]`
- **Bubbles v2 gained a `tree` component** (and textarea selection in v2.2.0), closing
  two long-standing gaps in the Charm kit. Both are new; the tree shows no known
  importers yet. `[BBL-PKG]` `[BBL-TREE]` `[BBL-TA]`
- **Lip Gloss v2 has a cell compositor with hit testing**, which overlaps the job
  bubblezone did. `[LG-PKG]` `[BZ-GH]`
- **Bubble Tea v2 moved to `charm.land/...` module paths** and the Charm kit now sits on
  Ultraviolet, a cell-diff renderer that needs no terminfo. `[BT-UP]` `[UV-GH]`
- **OpenTUI is the fastest-moving project** (releases on 2026-10-14, 22, 23, 25), has a
  full test renderer, a keymap package with layers and sequences, plugin slots,
  notifications, and a Diff component, and runs in production in OpenCode. It is
  pre-1.0 and tied to Bun or Node 26.4+ with an experimental FFI flag. `[OT-REL]`
  `[OT-GH]` `[OT-COMP]` `[OT-TEST]`
- **Ink 8.0.0** (2026-10-03) added `contentOffsetX/Y` on `<Box>` for scrollable views and
  `clientWidth/clientHeight` in `useBoxMetrics()`, and makes `useInput` drop unrecognised
  control sequences. Its official component library `@inkjs/ui` has been dormant since
  May 2024. `[INK-REL]` `[INKUI-DEPS]`
- **Brick 3.0 is new in September 2026**, with `Menu`, `MenuBar`, and `Animation`
  modules; its keybinding system is the most complete "events with names and
  descriptions, user overrides, collision detection, generated help" design I found.
  `[BRICK-HK]` `[BRICK-KD]` `[BRICK-KB-PRETTY]`
- **Terminal.Gui v2 is very broad** (a catalogue of over 50 views, including Markdown,
  Code, DatePicker, FileDialog, Wizard, PopoverMenu, Tabs, GraphView) and has the best
  documented semantic colour model. `[TG-VIEWS]` `[TG-SCHEME]`
- **Dormant staples:** blessed (2015), blessed-contrib (2022), Ratatouille (2020),
  Enquirer (2023), `@inkjs/ui` (2024), ink-table (2023). Jexer is archived. `[BLESSED-DEPS]`
  `[BC-DEPS]` `[RATA-HEX]` `[ENQ-DEPS]` `[INKUI-DEPS]` `[JEXER-GL]`
- **Security note:** pkg.go.dev flags Wish v2.0.0 (and only that version on the list I
  saw) with Go advisory GO-2026-5762 (SCP path traversal). Only relevant if Conspectus
  serves a TUI over SSH. `[WISH-VER]`

## 6. Unverified or weakly supported

- Discovery was not systematic (no web search). Missing projects are possible,
  particularly newer JS, Swift, OCaml, Nim and Crystal frameworks.
- tview: latest tag v0.42.0 (2025-08-27) comes from pkg.go.dev; the `deps.dev` pseudo-
  version list was truncated, so later commits are unverified. TextArea details are
  confirmed; `TreeView` details were cut off in the fetch and are not summarised.
- notcurses: release version, individual widget list (`ncselector`, `nctree`, ...), and
  last-commit date are unverified because the README does not describe them. I did not
  fetch the man pages.
- ncurses menu: the README-level page did not name `O_ONEVALUE` or `O_SHOWDESC`; I make
  no claim about checkbox-style menus.
- FTXUI: the `graph` element, gradients, and screen-reader support are not confirmed.
  Version 7.1.0 appears in the docs site header but I saw no dated release for it.
- libvaxis: tag year not shown (12 Nov); widgets listed from file names only.
- Spectre.Console: non-interactive fallback behaviour, Verify-style snapshot testing,
  and exception rendering were not confirmed (several doc pages returned 404).
- Hex1b, Melker, Raxol, nocterm: version, accessibility, and theming claims come from
  READMEs and are marked in the text where absent. Nocterm's "45+ components" claim
  (from `[AWESOME]`) was not confirmed on its own page.
- Evertras bubble-table: Bubble Tea v2 support and latest release unverified.
- Crush: whether its dialogs and palette are reusable components is unverified; the
  README treats it as an end-user tool.
- gum and Fang: latest tag numbers unverified for gum; the Fang colour-scheme API
  (`WithColorSchemeFunc`) is from pkg.go.dev only.
- Bubbles `cursor`, `timer`, and `stopwatch` were read from the README summary only.
- Ink year for release dates is confirmed by `deps.dev`. Clack, Inquirer, and FTXUI GitHub
  release dates carry day and month only; I take the year from the `deps.dev` or
  `pkg.go.dev` data when available and otherwise infer 2026.
- Star and commit counts are as shown on the GitHub pages when fetched and drift
  quickly.

## 7. Sources

All pages were fetched 2026-10-27.

**Charm (Go)**

- `[BT-REL]` https://github.com/charmbracelet/bubbletea/releases
- `[BT-PKG]` https://pkg.go.dev/charm.land/bubbletea/v2
- `[BT-VER]` https://pkg.go.dev/charm.land/bubbletea/v2?tab=versions
- `[BT-UP]` https://github.com/charmbracelet/bubbletea/blob/main/UPGRADE_GUIDE_V2.md
- `[BT-README]` https://github.com/charmbracelet/bubbletea/blob/main/README.md
- `[BBL-PKG]` https://pkg.go.dev/charm.land/bubbles/v2
- `[BBL-VER]` https://pkg.go.dev/charm.land/bubbles/v2?tab=versions
- `[BBL-KEY]` https://pkg.go.dev/charm.land/bubbles/v2/key
- `[BBL-HELP]` https://pkg.go.dev/charm.land/bubbles/v2/help
- `[BBL-LIST]` https://pkg.go.dev/charm.land/bubbles/v2/list
- `[BBL-TREE]` https://pkg.go.dev/charm.land/bubbles/v2/tree
- `[BBL-TA]` https://pkg.go.dev/charm.land/bubbles/v2/textarea
- `[BBL-TI]` https://pkg.go.dev/charm.land/bubbles/v2/textinput
- `[BBL-VP]` https://pkg.go.dev/charm.land/bubbles/v2/viewport
- `[BBL-TBL]` https://pkg.go.dev/charm.land/bubbles/v2/table
- `[BBL-FP]` https://pkg.go.dev/charm.land/bubbles/v2/filepicker
- `[BBL-PROG]` https://pkg.go.dev/charm.land/bubbles/v2/progress
- `[BBL-SPIN]` https://pkg.go.dev/charm.land/bubbles/v2/spinner
- `[BBL-PAG]` https://pkg.go.dev/charm.land/bubbles/v2/paginator
- `[LG-PKG]` https://pkg.go.dev/charm.land/lipgloss/v2 (also page 2 via offset)
- `[LG-VER]` https://pkg.go.dev/charm.land/lipgloss/v2?tab=versions
- `[LG-TABLE]` https://pkg.go.dev/charm.land/lipgloss/v2/table
- `[HUH-PKG]` https://pkg.go.dev/charm.land/huh/v2 (also the second half of the page)
- `[HUH-VER]` https://pkg.go.dev/charm.land/huh/v2?tab=versions and https://pkg.go.dev/github.com/charmbracelet/huh?tab=versions
- `[HUH-README]` https://github.com/charmbracelet/huh/blob/main/README.md and https://github.com/charmbracelet/huh
- `[GLAM-PKG]` https://pkg.go.dev/charm.land/glamour/v2
- `[GLAM-VER]` https://pkg.go.dev/charm.land/glamour/v2?tab=versions
- `[FANG-PKG]` https://pkg.go.dev/charm.land/fang/v2
- `[FANG-VER]` https://pkg.go.dev/charm.land/fang/v2?tab=versions
- `[HARM-GH]` https://github.com/charmbracelet/harmonica
- `[HARM-VER]` https://pkg.go.dev/github.com/charmbracelet/harmonica?tab=versions
- `[BZ-GH]` https://github.com/lrstanley/bubblezone
- `[BZ-VER]` https://pkg.go.dev/github.com/lrstanley/bubblezone/v2?tab=versions
- `[VHS-GH]` https://github.com/charmbracelet/vhs
- `[VHS-DEPS]` https://api.deps.dev/v3/systems/go/packages/github.com%2Fcharmbracelet%2Fvhs
- `[WISH-VER]` https://pkg.go.dev/charm.land/wish/v2?tab=versions (and https://pkg.go.dev/charm.land/wish/v2)
- `[UV-GH]` https://github.com/charmbracelet/ultraviolet
- `[CRUSH-GH]` https://github.com/charmbracelet/crush
- `[GUM-GH]` https://github.com/charmbracelet/gum
- `[X-GH]` https://github.com/charmbracelet/x
- `[TEATEST]` https://pkg.go.dev/github.com/charmbracelet/x/exp/teatest
- `[TEATEST2]` https://pkg.go.dev/github.com/charmbracelet/x/exp/teatest/v2
- `[GOLDEN]` https://pkg.go.dev/github.com/charmbracelet/x/exp/golden
- `[CPROF]` https://github.com/charmbracelet/colorprofile

**Other Go**

- `[NTC-GH]` https://github.com/NimbleMarkets/ntcharts
- `[NTC-DEPS]` https://api.deps.dev/v3/systems/go/packages/github.com%2FNimbleMarkets%2Fntcharts%2Fv2
- `[PTERM-GH]` https://github.com/pterm/pterm
- `[PTERM-DEPS]` https://api.deps.dev/v3/systems/go/packages/github.com%2Fpterm%2Fpterm
- `[STICK-GH]` https://github.com/76creates/stickers
- `[STICK-DEPS]` https://api.deps.dev/v3/systems/go/packages/github.com%2F76creates%2Fstickers
- `[EVT-GH]` https://github.com/Evertras/bubble-table
- `[TVIEW-GH]` https://github.com/rivo/tview
- `[TVIEW-PKG]` https://pkg.go.dev/github.com/rivo/tview (offsets 0 and 300000)
- `[TVIEW-VER]` https://pkg.go.dev/github.com/rivo/tview?tab=versions
- `[GOCUI-GH]` https://github.com/jesseduffield/gocui
- `[GOCUI-DEPS]` https://api.deps.dev/v3/systems/go/packages/github.com%2Fjesseduffield%2Fgocui

**JavaScript and TypeScript**

- `[INK-GH]` https://github.com/vadimdemedes/ink
- `[INK-README]` https://raw.githubusercontent.com/vadimdemedes/ink/master/readme.md
- `[INK-REL]` https://github.com/vadimdemedes/ink/releases
- `[INK-DEPS]` https://api.deps.dev/v3/systems/npm/packages/ink
- `[INKUI-GH]` https://github.com/vadimdemedes/ink-ui
- `[INKUI-TAGS]` https://github.com/vadimdemedes/ink-ui/tags
- `[INKUI-DEPS]` https://api.deps.dev/v3/systems/npm/packages/%40inkjs%2Fui
- `[DEPS-INK-ECO]` https://api.deps.dev/v3/systems/npm/packages/ink-testing-library , `.../ink-select-input` , `.../ink-text-input` , `.../ink-table` , `.../ink-spinner` , `.../ink-scroll-view` , `.../ink-form` , `.../pastel`
- `[OT-GH]` https://github.com/anomalyco/opentui
- `[OT-DOCS]` https://opentui.com/docs
- `[OT-COMP]` https://opentui.com/docs/components
- `[OT-SELECT]` https://opentui.com/docs/components/select
- `[OT-KEYMAP]` https://opentui.com/docs/keymap/overview
- `[OT-ADDONS]` https://opentui.com/docs/keymap/addons
- `[OT-TEST]` https://opentui.com/docs/core-concepts/testing
- `[OT-REL]` https://opentui.com/docs/releases
- `[OT-NOTIF]` https://opentui.com/docs/core-concepts/notifications
- `[OT-PS]` https://opentui.com/docs/application-apis/program-status
- `[OT-SLOTS]` https://opentui.com/docs/plugins/slots
- `[OT-COLORS]` https://opentui.com/docs/core-concepts/colors
- `[OT-INTERACT]` https://opentui.com/docs/core-concepts/interaction
- `[OT-DEPS]` https://api.deps.dev/v3/systems/npm/packages/%40opentui%2Fcore
- `[BLESSED-GH]` https://github.com/chjj/blessed
- `[BLESSED-DEPS]` https://api.deps.dev/v3/systems/npm/packages/blessed
- `[NEOB-DEPS]` https://api.deps.dev/v3/systems/npm/packages/neo-blessed
- `[BC-GH]` https://github.com/yaronn/blessed-contrib
- `[BC-DEPS]` https://api.deps.dev/v3/systems/npm/packages/blessed-contrib
- `[TK-GH]` https://github.com/cronvel/terminal-kit
- `[TK-DEPS]` https://api.deps.dev/v3/systems/npm/packages/terminal-kit
- `[CLACK-GH]` https://github.com/bombshell-dev/clack
- `[CLACK-README]` https://raw.githubusercontent.com/bombshell-dev/clack/main/packages/prompts/README.md
- `[CLACK-REL]` https://github.com/bombshell-dev/clack/releases
- `[CLACK-DEPS]` https://api.deps.dev/v3/systems/npm/packages/%40clack%2Fprompts
- `[ENQ-GH]` https://github.com/enquirer/enquirer
- `[ENQ-DEPS]` https://api.deps.dev/v3/systems/npm/packages/enquirer
- `[INQ-GH]` https://github.com/SBoudrias/Inquirer.js
- `[INQ-REL]` https://github.com/SBoudrias/Inquirer.js/releases
- `[INQ-CORE]` https://raw.githubusercontent.com/SBoudrias/Inquirer.js/main/packages/core/README.md
- `[INQ-TEST]` https://raw.githubusercontent.com/SBoudrias/Inquirer.js/main/packages/testing/README.md
- `[MELKER]` https://github.com/wistrand/melker

**C, C++, Haskell, Zig, Elixir**

- `[FTXUI-GH]` https://github.com/ArthurSonzogni/FTXUI
- `[FTXUI-REL]` https://github.com/ArthurSonzogni/FTXUI/releases
- `[FTXUI-COMP]` https://arthursonzogni.github.io/FTXUI/group__component.html
- `[FTXUI-DOM]` https://arthursonzogni.github.io/FTXUI/group__dom.html
- `[FTXUI-HPP]` https://raw.githubusercontent.com/ArthurSonzogni/FTXUI/main/include/ftxui/component/component.hpp
- `[NC-GH]` https://github.com/dankamongmen/notcurses
- `[NCURSES-ANN]` https://invisible-island.net/ncurses/announce.html
- `[NCURSES-FORM]` https://invisible-island.net/ncurses/man/form.3x.html
- `[NCURSES-PANEL]` https://invisible-island.net/ncurses/man/panel.3x.html
- `[TVISION-GH]` https://github.com/magiblot/tvision
- `[BRICK-HK]` https://hackage.haskell.org/package/brick
- `[BRICK-GH]` https://github.com/jtdaugherty/brick
- `[BRICK-GUIDE]` https://github.com/jtdaugherty/brick/blob/master/docs/guide.rst
- `[BRICK-CL]` https://raw.githubusercontent.com/jtdaugherty/brick/master/CHANGELOG.md
- `[BRICK-MB]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Widgets-MenuBar.html
- `[BRICK-MENU]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Widgets-Menu.html
- `[BRICK-FOCUS]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Focus.html
- `[BRICK-TABLE]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Widgets-Table.html
- `[BRICK-KD]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Keybindings-KeyDispatcher.html
- `[BRICK-KB-PRETTY]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Keybindings-Pretty.html
- `[BRICK-THEMES]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Themes.html
- `[BRICK-ANIM]` https://hackage-content.haskell.org/package/brick-3.0/docs/Brick-Animation.html
- `[VAXIS-GH]` https://github.com/rockorager/libvaxis
- `[VAXIS-W]` https://github.com/rockorager/libvaxis/tree/main/src/widgets
- `[VAXIS-VXFW]` https://github.com/rockorager/libvaxis/tree/main/src/vxfw
- `[VAXIS-REL]` https://github.com/rockorager/libvaxis/releases
- `[RATA-GH]` https://github.com/ndreynolds/ratatouille
- `[RATA-HEX]` https://hex.pm/packages/ratatouille
- `[RAXOL-GH]` https://github.com/Hydepwns/raxol

**.NET, JVM, Dart, lists**

- `[TG-GH]` https://github.com/gui-cs/Terminal.Gui
- `[TG-NUGET]` https://www.nuget.org/packages/Terminal.Gui/
- `[TG-VIEWS]` https://raw.githubusercontent.com/gui-cs/Terminal.Gui/develop/docfx/docs/views.md
- `[TG-KB]` https://raw.githubusercontent.com/gui-cs/Terminal.Gui/develop/docfx/docs/keyboard.md
- `[TG-SCHEME]` https://raw.githubusercontent.com/gui-cs/Terminal.Gui/develop/docfx/docs/scheme.md
- `[SPECTRE-GH]` https://github.com/spectreconsole/spectre.console
- `[SPECTRE-NUGET]` https://www.nuget.org/packages/Spectre.Console/
- `[SPECTRE-SITE]` https://spectreconsole.net/
- `[SPECTRE-TEST]` https://spectreconsole.net/console/how-to/testing-console-output
- `[SPECTRE-SEL]` https://spectreconsole.net/console/prompts/selection-prompt
- `[SPECTRE-MSEL]` https://spectreconsole.net/console/prompts/multi-selection-prompt
- `[SPECTRE-LAYOUT]` https://spectreconsole.net/console/widgets/layout
- `[HEX1B-GH]` https://github.com/mitchdenny/hex1b
- `[LANT-GH]` https://github.com/mabe02/lanterna
- `[LANT-MVN]` https://repo1.maven.org/maven2/com/googlecode/lanterna/lanterna/maven-metadata.xml
- `[JEXER-GL]` https://gitlab.com/AutumnMeowMeow/jexer
- `[MOSAIC-GH]` https://github.com/JakeWharton/mosaic
- `[KOTTER-GH]` https://github.com/varabyte/kotter
- `[NOCT-PUB]` https://pub.dev/packages/nocterm
- `[NOCT-GH]` https://github.com/Norbert515/nocterm
- `[AWESOME]` https://github.com/rothgar/awesome-tuis
