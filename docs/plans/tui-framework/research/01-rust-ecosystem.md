# Research 01: The Rust TUI Ecosystem

Evidence base for the gap analysis (`gap-analysis.md`). Scope: the controls,
conventions and design-system practices the Rust TUI ecosystem ships, so the
gap analysis can compare them with what Conspectus already has.

Researched 2026-10-27. Nothing was cloned, downloaded or executed.

## 0. Method, confidence, and how to read the source tags

- Version, release-date, download and repository facts come from the
  crates.io JSON API (`https://crates.io/api/v1/crates/<crate>`).
  "Last release" below is the crates.io `updated_at` (the latest upload).
  "Recent downloads" is crates.io's own recent-downloads counter (roughly the
  last 90 days). `docs.rs` pages show a build date that is often later than
  the crates.io upload date (for example ratatui-widgets 0.3.2 shows
  10 Sep 2026 on docs.rs but 2026-06-19 on crates.io); crates.io dates are
  used throughout.
- Ratatui compatibility comes from the crates.io dependency endpoint
  (`.../crates/<crate>/<version>/dependencies`), recorded as the declared
  requirement ("ratatui ^0.30" means it builds against ratatui 0.30.x).
  Where a crate depends on `ratatui-core` / `ratatui-widgets` instead, that is
  stated. A requirement alone does not prove the crate works; it only shows it
  declares compatibility.
- Behavior descriptions come from READMEs and docs.rs pages fetched through a
  summarizing fetch tool. Those summaries are secondary evidence: wherever the
  summary itself reported a gap or an inconsistency, it is carried through
  here and marked **unverified** or **inconsistent**.
- Source tags: a bracketed name like `[tui-popup]` in any table refers to the
  row of that name in section 5 (the crates table), which gives the crates.io
  URL and the README or docs URL used. Other URLs are written out in full.
- "Maturity" is an editorial label from release cadence, version number,
  downloads and declared stability, not a quality judgement: **core** (shipped
  by the ratatui project), **established** (multi-year, high downloads),
  **active** (recent releases, modest adoption), **young** (first release in
  2026, low downloads), **stale** (no release in more than a year),
  **experimental** (self-described or pre-0.1 or very low adoption).
- API-shape shorthand: **W** plain `Widget`; **SW** `StatefulWidget` with an
  app-owned `*State`; **H** headless state machine (no rendering, caller
  draws); **E** ships event handling; **T** has theming hooks.

## 1. Ecosystem facts that shape everything else

1. **Ratatui 0.30 is the current line and the ecosystem has moved to it.**
   ratatui 0.30.0 shipped 2025-12-26; 0.30.2 is current (2026-06-19). The
   crate is now a workspace: `ratatui-core` (traits, buffer, layout, style,
   text; "designed for maximum stability" so widget authors should depend on
   it), `ratatui-widgets` (the bundled widgets), backends `ratatui-crossterm`,
   `-termion`, `-termwiz`, `-termina`, and `ratatui-macros`. MSRV 1.86,
   `no_std` capable. `[ratatui]` (BREAKING-CHANGES, highlights v0.30,
   ARCHITECTURE).
   Sources: https://ratatui.rs/highlights/v030/ ,
   https://raw.githubusercontent.com/ratatui/ratatui/main/ARCHITECTURE.md ,
   https://raw.githubusercontent.com/ratatui/ratatui/main/BREAKING-CHANGES.md
2. **Almost every actively maintained third-party widget declares
   `ratatui ^0.30`** (or `ratatui-core ^0.1`): tui-tree-widget, tui-popup,
   tui-scrollview, tui-big-text, tui-prompts, tui-widget-list, tui-term,
   tui-logger, tui-markdown, throbber-widgets-tui, tachyonfx, edtui,
   ratatui-explorer, tui-file-explorer, tui-slider, ratatui-cheese, tui-pantry,
   rat-* (via `ratatui-core`), tuirealm, and tui-react (see section 5). The
   outliers are stale crates (section 6).
3. **Ratatui itself ships rendering primitives only.** Its docs say it
   "does not directly expose any event catching"; focus, event routing,
   theming, overlays-as-a-contract and forms are all left to the app or to
   third parties. Source: https://ratatui.rs/concepts/event-handling/
4. **The "official" org now hosts several second-party crates**:
   `ratatui/tui-widgets` (a monorepo for tui-popup, tui-scrollview,
   tui-big-text, tui-prompts, tui-scrollbar, tui-bar-graph, tui-box-text,
   tui-cards, tui-equalizer, tui-qrcode), `ratatui/ratatui-textarea`,
   `ratatui/ratatui-image`, `ratatui/tachyonfx`, `ratatui/ratzilla`,
   `ratatui/ansi-to-tui`, `ratatui/templates`.
   Sources: https://raw.githubusercontent.com/ratatui/tui-widgets/main/README.md ,
   crates.io repository fields for each.
5. **Two projects are "full alternatives" rather than ratatui add-ons**:
   cursive (own ncurses-style model) and iocraft (React-like); plus newer
   independent kits (FrankenTUI/`ftui`, `tuika`, AppCUI, `r3bl_tui`) that own
   their cell grid. The ratatui README lists cursive and iocraft under
   "Alternatives". Source: https://raw.githubusercontent.com/ratatui/ratatui/main/README.md
6. **A wave of very new crates (first release in 2026) targets exactly the
   "design system / component kit" niche**: ratatui-cheese, ratatui-interact,
   ratcn, tuika, tui-lipan, ratatui-kit, tui-nuances, ratatui-themekit,
   opaline, makeover-geometry, tui-overlay, ratatui-comfy-toaster/-tabs.
   Most have low adoption (hundreds to low thousands of recent downloads).
   This is the closest ecosystem analogue to the planned Conspectus kit, and
   none has established itself.

## 2. Control-type catalog

Ordered by the taxonomy in the brief, with extra control types found along
the way appended at the end. "Best-maintained" is the first listed provider.

### 2.1 Input and editing

| Control | Providers (best first) | API shape | Maturity | Notes and conventions |
| --- | --- | --- | --- | --- |
| Text input (single line) | `tui-input` [tui-input]; `tui-prompts` TextPrompt [tui-prompts]; `rat-text` text_input / text_input_mask / number_input / date_input [rat-text]; `ratatui-cheese` Input [ratatui-cheese]; `edtui` with `.single_line(true)` [edtui]; `ratatui-textarea` as single line (example) [ratatui-textarea]; `tui-dialog` [tui-dialog]; `ratatui-interact` Input [ratatui-interact]; tuirealm stdlib Input [tui-realm-stdlib] | `tui-input`: **H** (headless `Input`, caller renders, multiple backends: ratatui-crossterm default, crossterm, termion). `tui-prompts`: SW with `TextState`, builder `TextPrompt::from("Username")`, `Prompt::draw(frame, area, &mut state)`. rat-text: SW + `HandleEvent`. | tui-input: established (0.15.5, 2026-09-26, 608k recent). Others active to young. | tui-prompts documents readline/emacs bindings: Home/Ctrl+A, End/Ctrl+E, Ctrl+B/F, Backspace/Ctrl+H, Delete/Ctrl+D, Ctrl+K (kill to end), Ctrl+U (clear line), Enter completes, Esc/Ctrl+C aborts; render modes Password and Invisible; character soft-wrap; lacks bracketed paste, validation, default values, themes, multi-line (per its README roadmap). rat-text masks cover digits, hex/octal, letters, separators, and Java-DecimalFormat-style number patterns, with grapheme iteration and double-width support. ratatui-cheese Input adds placeholder, password mode, length cap, custom prompt and validation. |
| Multiline editor | `ratatui-textarea` (ratatui org, fork of tui-textarea) [ratatui-textarea]; `tui-textarea-2` (srothgan fork) [tui-textarea-2]; `rat-text` text_area (ropey-backed, millions of lines tested) [rat-text]; `tui-realm-textarea` [tui-realm-textarea]; `tui-textarea` (original, stale) [tui-textarea] | `ratatui-textarea`: `TextArea` owns all state and `&TextArea` implements `Widget` (not a separate State type); `TextArea::input()` takes native key events or backend-agnostic `Input`; direct methods (`delete_word`, `undo`, `copy`) for custom keymaps. | ratatui-textarea: active (0.9.3, 2026-10-05, 636k recent, 10 versions). tui-textarea: stale (0.7.0, 2024-10-22, still 707k recent downloads, depends on ratatui ^0.29). | ratatui-textarea features: undo/redo (50 edits by default, configurable, 0 disables), regex search (feature `search`, no search UI provided), emacs-style default bindings, selection with copy/cut/paste and a yank buffer (Ctrl+Y), optional line numbers, soft wrap (char and word modes), mouse scrolling, tab width, placeholder and password-masking examples, multiple textareas per screen; backends crossterm/termion/termwiz; optional serde. |
| Vim/modal editor | `edtui` [edtui] | `EditorState` + `EditorView` (builder, `render(area, buf)`) + `EditorEventHandler` (`default()` = Vim, `emacs_mode()`, custom `KeyEventHandler` registers) + `EditorTheme` builder. SW + E + T. | active (0.11.7, 2026-08-16, 135k recent). | Normal/Insert/Visual modes, motions (`hjkl wbe 0 _ $ gg G %`, `{}`, `f/t`), operators `d c y` with text objects (`diw`, `ci"`), `.` repeat, undo/redo, system clipboard (requires bracketed paste), syntect highlighting (feature), line wrap, absolute/relative line numbers, mouse (cursor placement, selection), optional `system-editor` feature that hands off to `$VISUAL`/`$EDITOR`. Explicitly "not a stand-alone code editor". |
| Code editor (highlighted) | `ratatui-code-editor` (tree-sitter) [ratatui-code-editor]; `edtui` syntect [edtui]; `rat-text` text_area [rat-text] | Widget with editor state. Details **unverified** beyond crate descriptions. | experimental (0.0.6, 2026-07-07) | tree-sitter vs syntect split is the notable choice. |
| Checkbox / Radio / Switch | `tui-checkbox` [tui-checkbox]; `rat-widget` checkbox, radio [rat-widget]; `ratatui-interact` CheckBox [ratatui-interact]; `tui-realm-stdlib` Checkbox (multi) and Radio (single) [tui-realm-stdlib]; `ratcn` Checkbox [ratcn]; `iocraft` Checkbox [iocraft]; `ratatui-cheese` Select/MultiSelect [ratatui-cheese]; `tui-lipan` Checkbox/Radio [tui-lipan]; AppCUI CheckBox/RadioBox/ThreeStateBox [appcui] | `tui-checkbox`: W with three style levels (`style`, `checkbox_style`, `label_style`), `LabelPosition` right/left/top/bottom, symbol presets (`[X]`, `[*]`, `(X)`), state passed in as a bool; the app handles input (Space toggles). Optional `theme` feature wires into tui-nuances. | tui-checkbox: young but fast-moving (0.5.1, 2026-10-14, 31 versions, 4.7k recent). | Ratatui has no checkbox. The tui-checkbox README lists Space to toggle, arrows or k/j to navigate in its demo. No provider ships a "Switch" as a distinct control (**not found**, not proven absent). AppCUI is the only one with a three-state box. |
| Select / Dropdown / Combobox | `rat-widget` choice, combobox [rat-widget]; `tui-realm-stdlib` Select (HTML-style dropdown; needs height 3 plus at least 3 rows for options) [tui-realm-stdlib]; `ratatui-interact` Select [ratatui-interact]; `ratatui-cheese` Select, MultiSelect [ratatui-cheese]; `ratcn` Select [ratcn]; `tui-prompts` SelectPrompt [tui-prompts]; `tui-lipan` Select [tui-lipan]; AppCUI DropDownList/ComboBox/Selector [appcui]; `sectioned-picker` (multi-select with non-selectable section headers) | Mostly SW with a state holding the option list and open flag. | Fragmented; none dominant. | No standalone, widely adopted dropdown crate exists. `ratatui-dropdown` named in the brief was **not found** on crates.io search results. Convention seen in rat-widget: popups are drawn by a `PopupCore` helper and need relocation handling when inside scrolling views. |
| Autocomplete | `tuika` CompletionPalette [tuika]; `tui-prompts` roadmap item only [tui-prompts]; `ftui` command_palette scorer [ftui-widgets] | Unverified for all. | young | No dedicated autocomplete widget found; nearest are palette/completion widgets. tui-prompts lists Autocomplete as an unchecked roadmap item. |
| Date/time picker | `rat-widget` date_input, calendar [rat-widget]; AppCUI DatePicker, TimePicker [appcui]; `tui-lipan` DatePicker [tui-lipan]; `ratatui-form` DateRangeBlock (YYYY-MM-DD fields) [ratatui-form]; `ratatui-calendar` crate name is a 0.0.0 placeholder reserved for "calendar and date-picker widgets" | rat-widget date_input is a masked text field driven by chrono format patterns. | young / placeholder | Date entry is covered almost only by the large kits. Ratatui's own `Monthly` calendar is display-only (section 2.2). tui-prompts lists Date as an unchecked roadmap item. |
| Slider / Range | `tui-slider` [tui-slider]; `rat-widget` slider [rat-widget]; `tuika` Slider [tuika]; cursive SliderView [cursive]; AppCUI HSlider/NumericSelector [appcui]; `tui-lipan` Slider [tui-lipan] | `tui-slider`: `SliderState::new(value, min, max)` (`increase`, `decrease`, `percentage`, `set_from_cell`/`cell` for mouse mapping); horizontal and vertical; 16 style presets; optional `handle_key`/`handle_mouse`. SW + E (feature `crossterm`) + T (tui-nuances). | tui-slider: young (0.4.6, 2026-10-26, 23 versions). | Key convention: arrows step, PageUp/PageDown move 10 steps, Home/End jump to extremes, click or drag sets value, wheel steps. Turning off the handle turns it into a progress bar. |
| Form layout and validation | `ratiform` [ratiform]; `ratatui-form` [ratatui-form]; `schemaui` (JSON Schema to TUI) [schemaui]; `jkconfig` (JSON Schema components) [jkconfig]; `rat-widget` form + layout [rat-widget]; `tuika` Form/FormState [tuika]; `tui-lipan`; `ratatui-interact` | `ratiform`: `FormBuilder` yields `FormState<T>` keyed by a user-defined enum (no string keys), `Form<T>` is a SW; `state.handle_input(key)`; `state.result()` returns `Submitted`/`Cancelled`/`Working`; dirty tracking; `.required(...)` rules; masked fields. `ratatui-form`: field types TextInput/Select/Checkbox plus composite Address/Contact/DateRange blocks; validators Required, Email, MinLength, MaxLength, Pattern (regex, with zip/phone/date constructors), custom `Validator` trait; `FormStyle::dark()/light()`; JSON export. | All young. schemaui is the most adopted (0.16.3, 37 versions, 120k total, 51k recent). | schemaui is the most complete reference: every field keeps its JSON pointer so validation errors map back; per-edit validation via a `FormEngine`; overlays stack for nested editors (Esc pops one level); a single keymap file (`keymap/default.keymap.json`) drives input handling, footer help, and README shortcut tables. Tab/Shift+Tab move between fields; Ctrl+S save; Ctrl+N/D add/remove collection items. |
| Wizard / Stepper | `ratatui-interact` StepDisplay (multi-step progress with sub-steps) [ratatui-interact]; `tui-lipan` Flowchart/StateDiagram are diagrams, not wizards | **Unverified**: no dedicated wizard control found. A crates.io search for "ratatui wizard stepper" returned zero crates. | n/a | Genuine ecosystem gap: wizard flows are built from forms and modal stacks by hand. |
| Undo / redo | `ratatui-textarea`, `edtui`, `rat-text` undo_buffer, `ftui-widgets` undo_support and history_panel [ftui-widgets] | Built into editors. | established for editors | Only FrankenTUI exposes undo as a separate widget concern (a history panel plus an `undo_support` module). |
| Clipboard | `hjkl-clipboard` (OSC 52 fallback over SSH, async) [hjkl-clipboard]; `rat-text` clipboard trait (pluggable; optional `cli-clipboard`) [rat-text]; `edtui` system clipboard [edtui]; `tui-panel-select` (panel-scoped mouse text selection and copy) [tui-panel-select]; `ratatui-comfy-toaster` returns `CopyRequested(String)` and leaves the copy to the app [ratatui-comfy-toaster] | Mostly a trait or a result type; the app performs the copy. | young | Convention worth noting: the comfy-toaster pattern (widget reports intent, app performs the effect) matches Conspectus's effect-lifecycle idea. No dominant clipboard crate in the ratatui ecosystem. |

### 2.2 Display and data

| Control | Providers (best first) | API shape | Maturity | Notes and conventions |
| --- | --- | --- | --- | --- |
| Paragraph / rich text / Block | `ratatui-widgets` Paragraph, Block [ratatui-widgets] | W. Paragraph: `Wrap { trim }`, `scroll((y, x))` (note y before x), alignment shortcuts, `Stylize`; unstable `line_count`/`line_width` behind `unstable-rendered-line-info`. Block: `bordered()`, `title/title_top/title_bottom` (take `Into<Line>` since 0.30), `Padding`, `border_type`, `merge_borders(MergeStrategy::{Replace,Exact,Fuzzy})`. | core | `Title` type was removed in 0.30; title alignment lives on `Line`. Six new dashed `BorderType`s in 0.30. Border merging enables collapsed-border layouts. Docs recommend the `textwrap` crate for complex wrapping. |
| List | `ratatui-widgets` List [ratatui-widgets]; `tui-widget-list` [tui-widget-list]; `rat-widget` list / `rat-ftable` [rat-widget]; `ratatui-cheese` List (paginated, `ListItem` trait) [ratatui-cheese]; `ratatui-kit` VirtualList [ratatui-kit]; `ftui-widgets` virtualized (variable row height) [ftui-widgets] | Built-in: SW with `ListState` (`offset`, single `selected`, `select_next/previous/first/last`, `scroll_down_by/up_by`); `highlight_symbol` takes `Into<Line>` since 0.30. **No multi-selection** in `ListState`. `tui-widget-list`: SW `ListView::new(ListBuilder, count)`, per-item closure gets `index` and `is_selected`, per-item sizes (returned number appears to be the size; **unverified**), `scrollbar`, `infinite_scrolling` (wrap-around), `scroll_axis` (horizontal lists), `scroll_padding`, `hit_test(col,row)` returning `Hit::Item(i)`/`Hit::Area`. | tui-widget-list: active (0.15.3, 2026-07-18, 157k recent). | Selection index is clamped at render time (the state stores `usize::MAX` for "last" until the next render). `hit_test` is the common mouse convention. |
| Table (basic) | `ratatui-widgets` Table [ratatui-widgets] | SW with `TableState`: row, column, and cell selection (`select_cell`, `select_next_column`, ...), `scroll_down_by`/`scroll_right_by`, serde on state (feature). | core | Cell-level selection is a built-in strength. No sort or filter UI. |
| Table with sort / filter / widget cells | `ratagrid` (typed data grid with sorting, search, selection, column resizing, pagination; mouse-first) [ratagrid]; `rat-ftable` (data-adapter Table) [rat-widget]; `ratatui-widgettable` (cells can hold any widget) [ratatui-widgettable]; `ratatui-kit` Table; `tuika` KeyedTable | Varies. | experimental: ratagrid 0.3.1 (4 versions, 96 downloads), ratatui-widgettable 0.1.2 (37 downloads) | No established sortable-table crate. Sort/filter is done in app code on top of `TableState` in most apps (**inference**, not verified across apps). |
| Virtual / infinite list | `tui-widget-list` (builder closure per item; infinite scrolling means wrap-around) [tui-widget-list]; `ftui-widgets` virtualized + height_predictor [ftui-widgets]; `ratatui-kit` VirtualList | see List | see List | "Infinite scrolling" in tui-widget-list is wrap-around navigation, not lazy loading. |
| Tree view | `tui-tree-widget` [tui-tree-widget]; `tui-realm-treeview` [tui-realm-treeview]; `ratatui-cheese` Tree [ratatui-cheese]; `ratatui-interact` TreeView [ratatui-interact]; `ratkit` [ratkit]; AppCUI TreeView [appcui]; `tui-lipan` Tree/FileTree [tui-lipan] | `tui-tree-widget`: SW with `TreeState<Identifier>` (Identifier is `Clone + Eq + Hash`; selection and open set are keyed by identifier path, not index): `key_up/down/left/right`, `toggle_selected`, `open/close/toggle`, `select_first/last/relative`, `click_at(position)`, `rendered_at(position)`, `scroll_up/down`, `close_all`. Items of different heights supported. | established (0.24.1, 2026-08-09, 1.09M recent, since 2020). | Key semantics: Left closes or moves to parent; Right opens. `rendered_at`/`click_at` implement hit testing from the last render. |
| Tabs | `ratatui-widgets` Tabs [ratatui-widgets]; `ratatui-comfy-tabs` [ratatui-comfy-tabs]; `tui-tabs` [tui-tabs]; `rat-widget` tabbed [rat-widget]; `ratatui-interact` TabView (four sides) [ratatui-interact]; `tuika` Tabs/TabSelect; cursive TabView is not in core views | Built-in Tabs: W with `select(Option<usize>)`, `highlight_style` (default reversed), `divider`, `padding`, `width()` (0.30), implements `UnicodeWidthStr`. No overflow scrolling or close buttons documented. | Tabs core. Third-party: young (comfy-tabs 0.5.12, tui-tabs 0.1.1). | Tabs only renders; selection is entirely the caller's. Overflow and closable tabs are left to the app (the page documents neither). |
| Accordion / Collapsible | `ratatui-interact` Accordion (one or many open) [ratatui-interact]; `tui-lipan` Accordion [tui-lipan]; AppCUI Accordion [appcui] | Unverified API. | young | Not provided by ratatui or any single-purpose crate found. |
| Pager / ScrollView / Scrollbar | `tui-scrollview` [tui-scrollview]; `tui-scrollbar` [tui-scrollbar]; `ratatui-widgets` Scrollbar [ratatui-widgets]; `rat-scrolled`/`rat-widget` view, clipper [rat-widget]; cursive ScrollView; iocraft ScrollView; `ratatui-interact` ScrollableContent | `tui-scrollview`: SW; `ScrollView::new(Size)`, `render_widget(widget, Rect)` into a larger virtual buffer, then `render(area, buf, &mut ScrollViewState)`; horizontal and vertical. `tui-scrollbar`: W on `&ScrollBar`, app-owned state; `ScrollMetrics` is pure geometry usable for tests and hit testing; `ScrollBarInteraction` stored in app state; `handle_mouse_event` returns a `ScrollCommand` the app applies; fractional 1/8-cell thumb; glyph sets `minimal`, `box_drawing`, `unicode`. Built-in Scrollbar: SW with `ScrollbarState::new(len).position(n)`, `get_position()` (0.30), four parts (begin, track, thumb, end), content length must be set before render. | tui-scrollview: active (0.6.8, 2026-09-24). tui-scrollbar: young (0.2.8, 530k recent via dependents). | Pattern: "return a command, let the app apply it" (tui-scrollbar `ScrollCommand`) separates input interpretation from state mutation. Conditional scrollbar display is unimplemented in tui-scrollview (README TODO). |
| Log viewer | `tui-logger` [tui-logger]; `ratatui-interact` LogViewer [ratatui-interact]; `ftui-widgets` log_viewer + log_ring [ftui-widgets]; tui-lipan LogView | `tui-logger`: `TuiLoggerWidget`, `TuiLoggerSmartWidget` (target selector on the left, message view on the right), `TuiWidgetState` fed `TuiWidgetEvent`s via `transition()`; integrations for `log`, `tracing-subscriber` (`TuiTracingSubscriberLayer`) and `slog`; env-filter style config; "hot" 1000-entry buffer feeding a 10000-entry main buffer; optional `waiter` feature for redraw signalling. SW + E-by-mapping (the app maps keys to events). | established (0.18.3, 2026-07-04, since 2018, 389k recent). | Per-target level filtering with five-letter level codes; the app must map keys itself (`h` hide selector, `f` focus target, PageUp/PageDown page mode, Esc exits page mode, `+`/`-` capture level). |
| Markdown / rich text | `tui-markdown` (markdown to `Text`) [tui-markdown]; `ratatui-markdown` [ratatui-markdown]; `tuika` Markdown (streaming) [tuika]; `mdstream` (streaming middleware); `ratatui-interact`; AppCUI Markdown | `tui-markdown`: `from_str(&str) -> Text`, `from_str_with_options(md, &Options)`; `StyleSheet` trait (heading(level), code, link, blockquote, table_*, alert_*, ...) with `DefaultStyleSheet`; fenced code highlighted by default (feature `highlight-code`, `Base16OceanDark`, TextMate themes loadable). | tui-markdown: active but self-described "experimental Proof of Concept" (0.3.12, 2026-10-19, 197k recent). | Handles headings, nested blockquotes, GFM alerts, tables (Unicode borders, CJK/emoji-aware width), task lists, footnotes; images become alt text, links render as `label (url)`, HTML shown dimmed. Output is plain `Text`, so scrolling is the caller's job. |
| Syntax-highlighted code view | `tui-syntax-highlight` [tui-syntax-highlight]; `syntect-tui` (syntect styles to ratatui styles) [syntect-tui]; `tui-markdown` code blocks; `tuika` CodeBlock; `edtui` highlighter | Converters producing styled `Text`/`Line`. | tui-syntax-highlight: young (0.2.0, 2025-12-26); syntect-tui: stale-ish (3.0.6, 2025-05-09) | Two engines in use: syntect (most) and tree-sitter (ratatui-code-editor). |
| Diff viewer | `ratatui-interact` DiffViewer (unified or side-by-side, hunk jumping, search, syntax highlight) [ratatui-interact]; `tui-lipan` DiffView [tui-lipan]; `tuika` Diff [tuika]; `vtcode-diff` (renderer-neutral preview rows) | Unverified API. | young | Diff viewers exist only inside the large kits; no standalone, widely used crate. |
| ANSI to styled text | `ansi-to-tui` [ansi-to-tui] | Function returning `Text`; depends only on `ratatui-core ^0.1`. | established (8.0.1, 2026-01-10, 3.42M recent). | Useful for embedding command output. |
| Embedded terminal (PTY) | `tui-term` [tui-term] | `PseudoTerminal` widget over a `vt100` screen; the app owns the PTY. | established (0.3.4, 2026-04-07, 381k recent). | Rendering only; key and mouse forwarding are the app's job (**unverified** README detail; the README could not be fetched). |
| Hyperlinks (OSC 8) | `hyperrat` [hyperrat] | W. | young (0.1.3, 2026-06-28) | Cell-width correctness of OSC 8 links is the stated selling point. |
| Image | `ratatui-image` [ratatui-image] | `Picker` (`from_query_stdio()` detects protocol and font size), `Image` (fixed size, resized once when the protocol is built) vs `StatefulImage` (adapts at render time, blocking), `ThreadProtocol` for off-thread resizing; `Resize::Fit` shown. | established (stable 11.1.0 2026-09-17; 12.0.0-rc.3 2026-10-18; 609k recent). | Protocols: Sixel, Kitty, iTerm2, halfblocks fallback. README lists non-working terminals (Alacritty, Konsole, Contour, Warp) and notes a termwiz backend bug. The rc depends on `ratatui ^0.30.1`. |
| Big text / QR | `tui-big-text` [tui-big-text]; `tui-qrcode` (in tui-widgets); `tui-box-text`; `ratatex` (LaTeX math via Kitty placeholders); `ratatui-splash-screen` | `BigText::builder()...build()`; `PixelSize` Full/HalfHeight/HalfWidth/Quadrant/ThirdHeight/Sextant/QuarterHeight/Octant; `to_text()` for tables/paragraphs. | tui-big-text: active (0.8.10, 2026-09-24). | Glyphs from `font8x8`; `to_text` clips glyphs wider than the area and does not wrap. |
| Progress bar / Gauge | `ratatui-widgets` Gauge, LineGauge [ratatui-widgets]; `ratatui-interact` Progress/StepDisplay; `ratatui-themekit` bar builder [ratatui-themekit]; cursive ProgressBar | W. 0.30 added customizable fill and empty symbols to LineGauge and deprecated `line_set`. | core | Gauge is a block bar with label; LineGauge is a single thin line. |
| Spinner / Throbber | `throbber-widgets-tui` [throbber-widgets-tui]; `tui-spinner` [tui-spinner]; `ratatui-cheese` Spinner (12 presets) [ratatui-cheese]; `tui-skeleton` (animated placeholders) [tui-skeleton]; `ratatui-interact` Spinner | `throbber-widgets-tui`: `Throbber` builder (`label`, `style`, `throbber_style`, `throbber_set`, `use_type`) with app-owned `ThrobberState`; the app calls `calc_next()` from its tick handler. Plain use is `render_widget` (random step); configured use is `render_stateful_widget`. | established (0.11.1, 2026-06-19, 264k recent). MSRV 1.88. | Animation is driven by the caller's tick; the widget has no clock. |
| Charts: Chart, BarChart, Sparkline, Canvas | `ratatui-widgets` [ratatui-widgets]; `tui-piechart` [tui-piechart]; `malevich` [malevich]; `tui-bar-graph`, `tui-equalizer` (tui-widgets); `ratatui-stacked-bar`; `tui-nodes`, `rataflow` (node graphs) [rataflow]; `tui-lipan` Chart/Heatmap/diagrams | Built-in: `Chart` (Datasets on cartesian axes, `LegendPosition`; GraphType variants **unverified**), `BarChart` (Bar/BarGroup, labels take `Into<Line>`), `Sparkline`, `Canvas` (Shapes: Circle, Line, Map, Points, Rectangle, FilledLine, Label; Braille default marker; 0.30 added Quadrant/Sextant/Octant markers; `Marker` is `#[non_exhaustive]`). | core | `malevich` claims "a small grammar of marks, honest axes, millions of points"; piechart has a high-resolution mode. |
| Calendar (display) | `ratatui-widgets` `calendar::Monthly` + `DateStyler` + `CalendarEventStore` [ratatui-widgets]; `rat-widget` calendar [rat-widget]; cursive `calendar_view` (third-party, listed in README) | W; `Monthly` shows the month containing `display_date`; surrounding days optional; styles from a `DateStyler`. | core | Feature flag: docs say "crate feature `calendar`" in one place and `widget-calendar` in another (**inconsistent**; `calendar` is in the default feature list). Display only; no selection. |
| Badge / chip / marquee / pagination | `ftui-widgets` badge, paginator [ftui-widgets]; `ratatui-cheese` Paginator [ratatui-cheese]; `ratatui-interact` MarqueeText | W | young | Paginator (dots or numeric) with slicing helpers is a ratatui-cheese idea. |
| Skeleton / loading placeholders | `tui-skeleton` (pulse, sweep, shimmer) [tui-skeleton]; `tui-shimmer`; `aisling` | W + animation | young (tui-skeleton 0.3.0) | Niche. |
| Node graph / diagrams | `rataflow` (pan, zoom, mouse) [rataflow]; `tui-nodes`; `tui-lipan` Graph, Flowchart, Sequence/Class/State/ER/Gantt diagrams [tui-lipan]; `boxmaid` (Mermaid as widgets) | Varies. | experimental | Outside the likely Conspectus control set but notable for completeness. |

### 2.3 Navigation, overlays, and feedback

| Control | Providers (best first) | API shape | Maturity | Notes and conventions |
| --- | --- | --- | --- | --- |
| Modal / Dialog / Confirm | `ftui-widgets` modal (ModalStack, Dialog presets, backdrop, focus trap) [ftui-widgets]; `ratatui-kit` Modal/ConfirmModal/AlertModal/ShortcutInfoModal [ratatui-kit]; `rat-widget` dialog_frame, msgdialog + `rat-dialog` (stack of dialogs above the app) [rat-widget]; `tuika` Dialog/Scene [tuika]; `tui-overlay` modals/drawers [tui-overlay]; `tui-popup` [tui-popup]; cursive Dialog [cursive]; AppCUI modal dialogs + message/input/open/save boxes [appcui]; `ratcn` Dialog [ratcn] | ftui: `ModalStack` (z-order and input routing), `StackModal` trait, `FocusAwareModalStack`, dialog presets with `DialogResult`, `BackdropConfig` (color and opacity, animatable), entrance/exit animations that respect reduced-motion, Esc closes. | ftui 0.9.0 (pre-1.0, nightly toolchain requirement stated in its README). No mature standalone crate. | Convention seen across kits: a stack of overlays that capture input first, a dimmed backdrop, focus restored to the previous owner on close, Esc to dismiss. Ratatui's own approach is the `Clear` widget plus a centered-rect helper (recipe "Popups (overwrite regions)"). |
| Popup / Tooltip / Popover | `tui-popup` [tui-popup]; `tui-overlay` [tui-overlay]; `ftui-widgets` popover (anchored) [ftui-widgets]; `tuika` popovers via `RectProbe` anchoring (flip and clamp) [tuika]; `ratcn` Tooltip [ratcn]; `tui-lipan` Popover/Tooltip [tui-lipan]; `ratatui-widgets` Clear [ratatui-widgets] | `tui-popup`: `Popup::new(content).title().style()`, centered and content-sized; `PopupState` adds keyboard moves (`move_up/down/left/right(n)`) and mouse drag (`mouse_down/up/drag`) (README calls this experimental); `KnownSize` / `KnownSizeWrapper` for content sizing. `tuika`: popovers resolve against terminal size with side, alignment, gap, flip when space runs out, clamp to margins. | tui-popup: active (0.7.7, 2026-09-24, 858k recent). | tui-popup unchecked roadmap: resize, border style choice, close button. Anchoring with flip/clamp (tuika) is the most complete placement model found. |
| Toast / Notification | `ratatui-comfy-toaster` (fork of ratatui-toaster) [ratatui-comfy-toaster]; `ratatui-toaster` [ratatui-toaster]; `ratatui-interact` Toast/ToastStack [ratatui-interact]; `ftui-widgets` toast + notification_queue [ftui-widgets]; `tuika` Toasts; `tui-lipan`; `ratkit`; `tui-overlay` | comfy-toaster: `ToastEngine<T>` built by `ToastEngineBuilder`; FIFO queue capped (`max_queue_depth`, default 4); five anchors plus offset; timed vs sticky (`keep_on`); dedup with `[4x]` counter; four levels (Info/Success/Warning/Error); depleting progress bar styles; `set_area_avoiding(area, &[rects])`; left click dismisses and right click requests copy on sticky toasts, returned as `ToastInteraction`; ids and `update_toast_by_id`; optional tokio expiry; wide characters measured correctly; animation planned (hook via `toast_area()` for tachyonfx). | young (comfy-toaster 0.6.2, 2026-07-18, 530 recent; toaster 0.1.4, 5k recent). | Dedup, avoid-rects placement and sticky/timed semantics are the design points worth copying. |
| Command palette / Fuzzy finder | `ftui-widgets` command_palette (Bayesian fuzzy scorer, action registry, `PaletteAction::Execute(id)`/`Dismiss`) [ftui-widgets]; `tuika` CompletionPalette [tuika]; `tui-lipan` SearchPalette [tui-lipan]; `hjkl-picker-tui` [hjkl-picker-tui]; `nucleo-picker` (fuzzy picker TUI library, not ratatui) [nucleo-picker]; `ratatui-hypertile-extras` (command palette in a tiling runtime) [ratatui-hypertile]; matcher `nucleo`/`nucleo-matcher` (2.1M recent) | Kits ship the widget; matching is usually delegated to `nucleo`. | young everywhere; the only ratatui-agnostic piece with scale is the matcher (`nucleo-matcher` 0.3.1, last release 2024-02-20). | No standalone, adopted ratatui command-palette crate exists. |
| File picker / explorer | `ratatui-explorer` [ratatui-explorer]; `tui-file-explorer` [tui-file-explorer]; `rat-widget` file_dialog [rat-widget]; `ftui-widgets` file_picker; `ratatui-interact` FileExplorer; AppCUI PathFinder + open/save dialogs [appcui] | `ratatui-explorer`: `FileExplorer::handle(&event)` plus `widget()` (rendered with `render_widget_ref`, which needs the unstable-widget-ref feature; **unverified** whether the crate enables it); `Theme` builder (`with_block`, `with_highlight_item_style`, ...); keys j/k/arrows, h/Left/Backspace parent, l/Right/Enter enter, Home/End/PageUp/PageDown, Ctrl+h toggles hidden. `tui-file-explorer`: three tiers (`FileExplorer`, `DualPane`, `App`), `handle_key` returns `ExplorerOutcome::{Selected(path), Dismissed, ...}`, `render_themed(&mut explorer, frame, area, &Theme)`, remappable `keymap.toml`, incremental search (`/`), recursive fuzzy finder (`f`), multi-select with marks, sort modes, 43 theme presets, previews, undo/redo of file ops. | ratatui-explorer: established-ish (0.3.0, 2026-03-06, 49k recent, since 2024). tui-file-explorer: active (3.2.0, 2026-10-25, 79 versions, Rust 1.90 minimum). | The outcome enum (`Selected`/`Dismissed`) is a clean widget-to-app contract. |
| Menu bar / Context menu | `tui-menu` [tui-menu]; `rat-menu` (MenuLine, PopupMenu, Menubar) [rat-menu]; `ratatui-interact` MenuBar and ContextMenu (right-click, nested submenus, shortcut hints) [ratatui-interact]; cursive Menubar [cursive]; AppCUI Menu/PopupMenu/CommandBar [appcui]; `tui-lipan` ContextMenu | `tui-menu`: SW; `MenuState::new(vec![MenuItem::group(label, children), MenuItem::item(label, data)])`; `drain_events()` yields `MenuEvent::Selected(data)`; `reset()` closes; render last so it overlays. | tui-menu: stale-ish (0.3.1, 2025-12-30, 2.3k recent, 9 releases). rat-menu: active (3.1.1, 2026-03-08). | rat-menu depends on rat-event, rat-focus, rat-cursor and rat-reloc (and probably rat-popup, which the docs page only implies), so it is not usable alone in practice. Mnemonic/hotkey navigation and popup placement were **not documented** on the page fetched. |
| Breadcrumb | `ratatui-interact` Breadcrumb (collapses with an ellipsis; keyboard and mouse) [ratatui-interact]; `tui-lipan` Breadcrumb [tui-lipan]; schemaui uses breadcrumb-style section titles [schemaui] | Unverified API. | young | No standalone crate. |
| Split pane (resizable) | `ratatui-hypertile` (tiling engine, mouse drag of borders, save/restore) [ratatui-hypertile]; `rat-widget` splitter [rat-widget]; `ratatui-interact` SplitPane (draggable divider) [ratatui-interact]; `ftui` pane workspaces (drag-resize, docking, undo/redo) [ftui]; `tui-lipan` Splitter; `ratkit` | `ratatui-hypertile` core: takes an area and returns rectangles; tracks tree, focus and movement; `split_focused(Direction)`, `compute_layout(Rect)`, `panes_iter()` with `is_focused`. Extras crate adds plugins, workspace tabs, vim-style keymaps, command palette. | young (0.4.2, 2026-10-01, 4.4k recent). | The engine returns rectangles and leaves drawing to the app, a clean separation. Not a BSP tree as far as the README states (**unverified**). |
| Key-hint / Status bar / Help overlay | `ratatui-cheese` Help (compact or expanded multi-column keybinding view) [ratatui-cheese]; `tuika` KeyHints, keymap-driven [tuika]; `rat-widget` statusline, statusline_stacked [rat-widget]; `ftui-widgets` help, help_registry, hint_ranker, status_line [ftui-widgets]; `ratatui-interact` HotkeyDialog; `tui-lipan` StatusBar; AppCUI CommandBar; `ratatui-themekit` status line builder | tuika: bindings are declarative, support chords and multi-stroke sequences, are grouped into prioritized, mode-gated layers, and can be queried to drive help and key hints; character chords match logical text so the keyboard layout is respected. schemaui: one keymap JSON drives input, footer and docs. | young everywhere; the pattern matters more than any crate. | This is the closest ecosystem match to Conspectus's "key tables that generate help": tuika, schemaui and tui-file-explorer (`keymap.toml`) all do it. `crokey` (1.5.1, 2026-10-17, 2.4M recent) parses and describes key combos for config-driven bindings. |

### 2.4 Layout, effects, and cross-cutting infrastructure

| Control / capability | Providers | API shape | Maturity | Notes and conventions |
| --- | --- | --- | --- | --- |
| Layout (constraints, Flex) | ratatui `layout` module [ratatui-widgets]; `rat-widget` layout calculators + `GenericLayout` for forms [rat-widget]; `ftui-layout` flex and grid solvers [ftui]; `tuika` integer flexbox subset with Flow and Grid [tuika]; `iocraft` flexbox via taffy [iocraft] | Ratatui: Cassowary solver (via the `kasuari` crate); `Constraint::{Min, Max, Length, Percentage, Ratio, Fill}` with priority Min highest to Fill lowest; `Flex::{Start, End, Center, SpaceBetween, SpaceAround, SpaceEvenly, Legacy}`; `Spacing`, `Margin`, `Offset`, `Layout::areas/split`; layout cache enabled by default in `ratatui`, opt-in in `ratatui-core` (`layout-cache`). 0.30 added `Rect::layout/try_layout/layout_vec`, `centered*`, `outer`, `Flex::SpaceAround` now follows CSS flexbox (old behavior is `SpaceEvenly`), `Alignment` renamed `HorizontalAlignment`. | core | Source: https://docs.rs/ratatui/latest/ratatui/layout/index.html and the 0.30 highlights. The layout module listing has no dedicated grid type (a "Grid Layout" recipe exists on https://ratatui.rs/recipes/, built from nested layouts; recipe body not read). |
| Animation / effects | `tachyonfx` [tachyonfx]; `ftui` modal/animation + `animate-core` (spring and tween) | `tachyonfx`: effects mutate cells after widgets render. Catalog: `fade_from/to(_fg)`, `hsl_shift`, `dissolve`/`coalesce`, `slide_in/out`, `sweep_in/out`, `translate`, `resize_area`, `effect_fn`, timing (`ping_pong`, `repeat`, `sleep`, `with_duration`), composition (`parallel`, `sequence`), `CellFilter` (`FgColor`, `Outer`, `Text`, `AllOf`) via `with_cell_selection`, `EffectTimer` with `Interpolation` easing, `EffectManager` (ids replace/cancel), a `dsl` module, `pattern`, `wave`, helper widgets; `sendable` feature. | established (0.25.2, 2026-09-06, 250k recent). | The shader-over-buffer model composes with any widget set; its README snippet still shows `0.7.0` (stale README). |
| Mouse support | ratatui-interact (`ClickRegionRegistry`); tui-widget-list `hit_test`; tui-tree-widget `click_at`/`rendered_at`; tui-scrollbar `handle_mouse_event`; tui-popup drag; tui-slider; edtui; rat-event `MouseOnly`/`DoubleClick` qualifiers; ratcn engine hover; tui-lipan MouseRegion; ftui hit-test regions | Common pattern: record rects during render, hit-test on mouse events, return a typed result. | n/a | Mouse is always opt-in and backend-level (capture must be enabled). No two crates share a hit-test type. |
| Focus management | see section 3.1 | | | |
| Theme / design tokens | see section 3.4 | | | |

## 3. Framework-level conventions

### 3.1 Focus management

| Approach | Where | Shape |
| --- | --- | --- |
| None provided; app-owned | ratatui | Docs say event handling is left to the app and show three patterns: centralized `match`, centralized with message passing, distributed event loops. https://ratatui.rs/concepts/event-handling/ |
| Passive flag per widget plus a rebuilt `Focus` | rat-focus 2.1.1 [rat-focus] | `FocusFlag` lives in each widget state; widgets implement `HasFocus` (macro `impl_has_focus`); `FocusBuilder` collects the ordered widget list, rebuilt per event so it never goes stale; `Focus` offers `next`, `prev`, `focus_at`, handles events via `.handle(event, Regular)`; macros `on_gained!`, `on_lost!`, `match_focus!`. Containers and modal focus were **not documented** on the fetched page. |
| Generic focus manager keyed by app enum | ratatui-interact [ratatui-interact] | `FocusManager<T>` with `register`, `next()`, `prev()`, `is_focused`, `focus`; `ClickRegionRegistry` for mouse hit-testing; traits `Focusable`, `Clickable`, `Container`; Tab/Shift+Tab. |
| Navigation graph with traps and spatial navigation | ftui-widgets focus [ftui-widgets] | `FocusGraph`/`FocusNode`/`NavDirection`, `FocusManager` with history, `FocusGroup`, `FocusTrap`, `spatial_navigate` for arrow-key moves, `FocusIndicator` styling. Modal stack auto-focuses the first element, confines Tab, restores focus on close. |
| Policies and scopes | tui-lipan [tui-lipan] | `OnDemand`/`Auto`/`Manual` policies, tab stops, scopes, focus events; focused widget receives keys first and unhandled ones bubble child to parent to root; Tab/Shift+Tab in DOM order. |
| Registry + router delivering events to the owning surface | tuika [tuika] | Scenes call `sync_focus`; `Router` delivers every event kind (including paste) to the surface that owns input and reports `Delivery` (who received what, or nothing); dialogs name a focus owner; `FormState` handles traversal. |
| Single-focus View | tuirealm [tuirealm] | `View` mounts/unmounts components, holds focus and forwards events; subscriptions (`Sub`, `SubClause`, `IsMounted`) provide global listeners. |
| Input layers | ratatui-kit [ratatui-kit] | `InputRuntime`, `InputLayer`, `EventScope`, `EventPriority`, `EventResult` so modals and edit modes block background shortcuts. |

Conventions to note: the focus systems that support overlays model "input
capture" explicitly (rat-event `Dialog` qualifier, ftui `FocusTrap`, ratatui-kit
`InputLayer`/`EventPriority`, tuika dialog focus owner). rat-focus rebuilds its
widget list on every event and tuika re-syncs through `sync_focus` rather than
caching a tree; tui-lipan instead keeps node identity stable with keys
(the other systems' behavior on this point was not documented in what was read).

### 3.2 Event routing

- **rat-event 2.1.0** [rat-event]: `HandleEvent<Event, Qualifier, Return>` with
  `Return: ConsumedEvent`; qualifiers `Regular`, `MouseOnly` (replace key
  bindings but keep mouse), `DoubleClick`, `Dialog` (runs first and consumes
  everything), and `Popup` (marked obsolete); `Outcome` says whether a
  re-render is needed and results convert to and from it; `event_flow!` macro
  chains handlers; `ct_event!` builds crossterm patterns. Only crossterm is
  implemented.
- **rat-salsa 4.0.3** [rat-salsa]: event loop that polls multiple sources
  fairly, converts each event into an app-defined type, and calls a handler
  returning `Control`; `SalsaContext::add_timer`, `spawn` (worker threads),
  `spawn_async` (async tasks), focus via `SalsaContext::focus`, an
  autonomous dialog stack via `rat-dialog`.
- **tuirealm 4.1.0** [tuirealm]: Elm-style `update`; `Component` and
  `AppComponent` traits; `Cmd` sent from an `AppComponent` to a `Component`;
  `Event` enum with user events; listener/ports (`async-ports` feature);
  `Sub`/`SubClause` subscriptions; a `testing` module.
- **tui-lipan** [tui-lipan]: components with `Message`, `Properties`, `State`,
  `view -> Element`, `update -> Update` that may return a `Command` run in the
  background; keyed identity keeps focus stable across reorders.
- **ratcn** [ratcn]: `Ratcn::render` and `Ratcn::handle_event` are the only
  two integration calls; the engine reports user actions back as messages.
- **Keymaps**: tuika (declarative, chorded, layered, mode-gated),
  schemaui (JSON), tui-file-explorer (`keymap.toml`), crokey (key parsing).
  `terminput` (0.5.15, 885k downloads) abstracts input events across
  backends.

### 3.3 Overlay, popup and modal contract

- Ratatui: draw a `Clear` over the area, then the overlay, in render order
  (`Clear` "erases its area, useful for overlays"). Source:
  https://ratatui.rs/concepts/widgets/ . A dedicated "Popups (overwrite
  regions)" recipe exists (page listed in https://ratatui.rs/recipes/ ; its
  content could not be fetched, so details are **unverified**).
- Kits formalize it as a **stack/scene of overlays** that own input first:
  ftui `ModalStack`; tuika `Scene` + `SceneOverlay` + `OverlaySpec` resolved
  against terminal size on every render; rat-dialog stack; tui-lipan modals,
  popovers and toasts with dismissal policies and focus capture.
- Placement features found only in tuika: anchor to a trigger via `RectProbe`,
  side and alignment, flip when out of room, clamp to margins. Placement
  features in comfy-toaster: `set_area_avoiding(rects)`.

### 3.4 Theming and design tokens

| Approach | Crate | Detail |
| --- | --- | --- |
| No theming in the toolkit | ratatui | 0.30 adds only serde for palette types and `Style` shorthand in const contexts; no theme type. https://ratatui.rs/highlights/v030/ |
| Semantic color slots + `Themed` trait + contrast math + color-depth downgrade | tui-nuances 0.1.10 [tui-nuances] | One `Theme` with about 30 slots in three groups (UI: foreground, muted, border, border_focused, primary, selection; Status: success/warning/error/info; Data-viz: palette, bull, bear, axis, grid, crosshair, tooltip). Style methods `style()`, `muted_style()`, `border_style(focused)`, `selected_style()`, `disabled_style()`, `status_style(Status)`. Widget crates implement `Themed::apply_theme`; apps implement `ThemeProvider`. `color` module: mix, lighten, relative luminance, contrast ratio, `ensure_contrast`. `ColorSupport::detect()` reads `COLORTERM`, `TERM`, `NO_COLOR`; `Theme::downgrade`. README claims every theme except `default`/`mono` passes WCAG AA for text and 90 palettes (its own breakdown sums to 86, so **inconsistent**). Consumers listed: tui-chart, tui-slider, tui-spinner, tui-rating, tui-piechart, tui-checkbox, tui-file-explorer (explicit `theme` colors always override the theme, regardless of call order). |
| Semantic slots + state-aware styles + builders + NO_COLOR theme | ratatui-themekit 0.6.1 [ratatui-themekit] | About 20 slots (accent, accent_dim, text/text_dim/text_bright, success/error/warning/info, diff_added/removed/context, border, surface, background) plus derived `block_*`/`indicator_*`; `ss.resolve(is_focused, is_selected, is_disabled)`; `ThemeExt` builders for spans, lines, blocks (`.focused(true)`), status lines, bars, table/list/tab/gauge/input/scrollbar/notification style bundles, zebra rows; 11 themes; `resolve_theme` returns a `Color::Reset` NoColor theme when `NO_COLOR` is set. |
| TOML palette to tokens to styles to gradients, strict resolver | opaline 0.4.2 [opaline] | Pipeline TOML to `ThemeFile` to `Resolver` to `Theme`; cycles, unresolved tokens and bad colors are load-time errors; contract of 28 core tokens, 14 styles, 5 gradients checked by tests; token namespaces `text.*`, `bg.*`, `accent.*`, `border.*`, `code.*`; 39 built-in themes in 17 families embedded with `include_str!`; optional discovery of user themes under `~/.config`; `ThemeSelector` widget (live preview, search, cancel/restore, feature `widgets`); ratatui adapter feature. The README did not mention NO_COLOR or automatic downgrade. |
| Style-name registry + persistable palette | rat-theme4 4.5.3 [rat-theme4] | `SalsaTheme` maps a flat style name to a ratatui `Style` or a composite style for rat-widget widgets; `StyleName` trait adds named constants (`Style::SELECT`, `Style::INPUT`), `WidgetStyle` anchors composite names (`WidgetStyle::CHECKBOX`); `Palette` (colors plus aliases) is the persistable part (`.pal` files via `load_palette`/`store_palette`); shipped palettes and themes. About 55% documented per docs.rs. |
| `Copy` token struct passed via render context + `StyleSheet` of semantic roles | tuika [tuika] | `Theme` is a plain `Copy` struct of color tokens; swapping restyles the tree; presets constants and `themes::by_name`/`PRESETS`; terminal-palette inheritance is opt-in and never probed unless asked; `StyleSheet` maps roles (heading, link, panel border...) to styles with namespaced custom roles and a `StyleResolver`. |
| Shared `Palette` + per-component `FooTheme` + overrides | ratatui-kit [ratatui-kit], ratatui-cheese [ratatui-cheese] | `PaletteProvider` recolors globally, `ThemeOverride` targets one component type, palette can be driven by an atom for runtime re-theming (ratatui-kit). ratatui-cheese: "All widgets derive their styles from a shared `Palette`" with five presets. |
| Presets, contrast policies, `system` theme, TOML hot reload | tui-lipan [tui-lipan] | Feature `theme-reload` hot-reloads TOML theme files; focus decoration is configurable. |
| Named style registry | ftui-style/ftui-widgets [ftui-widgets] | `StyleSheet` registry of named styles and a `style_names` module of standard names; "CSS-like cascading" claimed by the project README. |
| **Non-color design tokens** | makeover-geometry 0.8.3 [makeover-geometry] | The only system found that tokenizes geometry: relational spacing `Gap` (pointer 4, peer 6, group 10, section 12, pane 24, page 32 in its base units), `Radius`, `Text` roles (body/note/head), two stroke weights, `Density` (Pointer/Touch), `SizeClass` (Material 3 breakpoints 600/840), and a `Surface` axis; `Surface::terminal` resolves to whole cells ("0, 0, 1, 1, 2, 2": bound and peer collapse to zero because adjacency conveys the relationship). Colour lives in a sibling crate (`makeover`); a theme "overrides colour tokens only". |
| Terminal capability detection | termprofile 0.2.4 [termprofile] | Detects color/style support and converts ratatui colors and styles accordingly. |
| Palette sources | `catppuccin` 2.8.0 (ratatui feature, `ratatui-core`) [catppuccin]; `ratatui-themes` 0.3.0 (16 named themes, six core slots `accent, secondary, bg, fg, muted, selection` plus four status slots; serde as kebab-case names) [ratatui-themes]; `ratatui-base16` (stale, 2024-12-21); `chromata` (1000+ editor themes as constants) | |
| Preview chrome | tui-pantry [tui-pantry] | Dark/light chrome from Catppuccin Mocha/Latte; named preview backgrounds; color-depth emulation cycling 24-bit, 256, 16, 8, mono. |

Observation: the ecosystem converged on **semantic color slots** (success,
warning, error, info, muted, border, selection, accent), **state-aware style
resolution** and **NO_COLOR/downgrade handling** being separate concerns.
Spacing, density, borders and glyph choice are almost never tokenized
(makeover-geometry is the exception).

### 3.5 Testing and previewing

| Practice | Where | Detail |
| --- | --- | --- |
| In-memory backend with buffer assertions | ratatui `TestBackend` | `assert_buffer`, `assert_buffer_lines`, `assert_scrollback(_lines)`, `assert_scrollback_empty`, `assert_cursor_position`, `with_lines`, `resize`; error type `Infallible` since 0.30. https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html |
| Snapshot with insta | ratatui recipe | `Terminal::new(TestBackend::new(80, 20))`, `terminal.draw(..)`, `assert_snapshot!(terminal.backend())`, review with `cargo insta review`; fixed terminal size recommended; **colors cannot be asserted** (ratatui issue #1402). https://ratatui.rs/recipes/testing/snapshots/ ; `insta` 1.49.0 (2026-10-03, 35M recent) [insta] |
| PTY end-to-end | `termlens` [termlens], `termwright` [termwright], `termproof` [termproof], `ratatui-testlib` [ratatui-testlib] | The ratatui snapshot recipe itself recommends `termlens` for the event loop, key handling, setup/teardown and exit codes: `Terminal::builder().size(80, 24).spawn(...)`, `wait_until(|screen| ...)`, `send(Key::Char('j'))`, `insta::assert_snapshot!(terminal.screen())`. `termwright` is a "Playwright-like" framework (two releases). `termproof` is "evidence-first verification" with recipes and an evidence pipeline. |
| Widget storybook | tui-pantry 0.4.0 [tui-pantry] | `Ingredient` trait (`group`, `name`, `source`, `render(area, buf)`; optional `props`, `interactive`, `handle_key`, `handle_mouse`); four tabs (Widgets, Panes, Views, Styles); `pantry.toml` registers modules; `cargo pantry list` / `dump` render headless to ANSI (default 80x24, `--variant`, `--size WxH`) for CI; color-depth emulation and background cycling. No built-in snapshot comparison. |
| Resize sweeps and scripted loops | tuika testing module [tuika] | `render`, `render_with_sheet`, `grid` (plain-text snapshot), `render_sizes` (resize sweep), `TestHarness`, `render_app`, `step_app`, `run_driven_by` with `scripted_events`, pluggable `Clock` for determinism. |
| UI snapshot capture in several formats + automation | tui-lipan [tui-lipan] | `capture_ui_snapshot()` as Markdown, PNG or JSON; `TUI_LIPAN_SNAPSHOT` env var captures an unmodified app; `Sketch` renders a view at several viewport sizes; Unix-socket automation protocol; `F12` devtools overlay. |
| Demo screens double as snapshot tests | ftui [ftui] | Refresh with `BLESS=1`; `ftui-harness` shadow-run frame checksums, PTY tests, perf-regression gates (project's own claims). |
| Library-level test aids | tuirealm `testing` module [tuirealm]; ratatui-kit `test-util` feature [ratatui-kit]; tui-scrollbar `ScrollMetrics` (pure geometry usable without rendering) [tui-scrollbar]; opaline contract tests (28 tokens, 14 styles, 5 gradients) [opaline] | |
| Independence enforcement | ratcn [ratcn] | Components are copied into the app (`cargo ratcn add dialog` copies source to `src/components/`); a `copy-fixture` crate builds every component from a copy to prove they do not depend on each other. Interesting precedent for Conspectus's "compiler rejects app references" goal, from the opposite direction. |

### 3.6 Unicode, color depth, accessibility

- Width handling: ratatui `Tabs` implements `UnicodeWidthStr`; `width_cjk()` is
  behind the `cjk` feature and the docs call it "probably the wrong choice for
  most terminals". 0.30 allows `unicode-width` 0.2.1 (two behavior changes
  linked in BREAKING-CHANGES) and stores `Cell::symbol` as an
  `Option<CompactString>`; wide VS16 emoji are cleared properly. rat-text
  iterates graphemes and supports double-width characters; tui-markdown
  tables measure CJK/emoji correctly; comfy-toaster measures wide characters.
- Color depth: tui-nuances `ColorSupport::detect` (`COLORTERM`, `TERM`,
  `NO_COLOR`), termprofile, ratatui-themekit NoColor theme, tui-pantry depth
  emulation, tachyonfx `term256_colors` effect.
- Accessibility: only `ftui-a11y` 0.9.0 addresses it: an `Accessible` trait,
  an accessibility tree with diffing, `ContrastProfile`/`MotionProfile`
  preferences (reduced motion, high contrast, forced colors); the
  screen-reader bridge is described as not yet implemented. tui-lipan lists
  "contrast policies"; tui-nuances claims WCAG AA text contrast. No other
  crate reviewed documents accessibility.

## 4. Application and component frameworks compared

| Framework | Ratatui relation | Model | Widgets | Focus / events | Theming | Testing | Maturity (as of 2026-10-27) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| rat-salsa + rat-widget family [rat-salsa] [rat-widget] | on ratatui-core/widgets | event queue, timers, worker threads, async, dialog stack; widgets are plain `StatefulWidget` + `HandleEvent` | 40 modules (button, calendar, checkbox, choice, combobox, date_input, dialog_frame, file_dialog, form, list, menu, msgdialog, number_input, radio, slider, splitter, statusline, tabbed, table, text_input(_mask), textarea, toolbar (experimental), view, clipper, shadow...) | rat-focus, rat-event, rat-reloc (widgets that move rendered image invalidate stored areas), rat-scrolled | rat-theme4 | not covered | rat-salsa 4.0.3 (2026-03-08, ~2k recent), rat-widget 3.2.1 (2026-03-09, 4.5k recent), rat-focus/rat-event ~37k recent. Repo has 3,329 commits, 65 stars. The repo already rejected adopting it (CSP-433) for the mandatory trifecta. |
| tuirealm + stdlib [tuirealm] [tui-realm-stdlib] | on ratatui ^0.30 | React/Elm components with props and state, messages | Stdlib: BarChart, Canvas, Chart, Checkbox, Container, Gauge, Input, Label, LineGauge, List, Paragraph, Phantom, Radio, Select, Span, Sparkline, Spinner, Table, Textarea (plus separate treeview and textarea crates) | single-focus `View`, subscriptions, ports | props on components | `testing` module | active (4.1.0, 2026-05-02; 4.0.0 on 2026-04-18; 44k recent; 258k total) |
| cursive [cursive] | independent (own backends: crossterm default, others) | callbacks on views, layered screen | rich view set (Button, Checkbox, Dialog, EditView, ListView, Menubar, Panel, ProgressBar, RadioGroup, ScrollView, SelectView, SliderView, StackView, TextArea, TextView, ThemedView...) plus third-party tabs, tree, calendar, table | `CircularFocus`, `FocusTracker` | TOML/palette themes (basic 8-color widely supported; custom colors "kinda experimental") | not covered | **stale release**: 0.21.1 on 2024-08-03; repo shows 1,836 commits, 191 open issues, no archive banner |
| iocraft [iocraft] | independent (README credits ratatui as a similar project) | declarative `element!` macro, hooks (`use_state`, `use_future`), taffy flexbox | View, Text, MixedText, TextInput, Button, Checkbox, ScrollView, Fragment, ContextProvider | components handle Enter/Space/click | border styles, colors | not covered | active (0.9.3, 2026-10-14, 55k recent) |
| ratatui-kit [ratatui-kit] | ratatui ^0.30.1 + tokio | React-like components, hooks, router, atoms | View, Border, Text, ScrollView, Select, MultiSelect, Button, Modal family, Input, TextArea, TreeSelect, Table, VirtualList | InputLayer/EventScope/EventPriority | Palette + FooTheme | `test-util` feature | young (0.12.0, 2026-10-11, 36 versions, 2.3k recent) |
| tui-lipan [tui-lipan] | ratatui ^0.30 inside; "no ratatui types in public API" | Elm/React components with reconciliation, MPL-2.0 | very wide (layout, forms, DatePicker, SearchPalette, Chart, diagrams, Modal, Popover, Toast, Accordion, ContextMenu, DiffView...) | policies, scopes, bubbling | presets, hot-reload | snapshots, automation | young but fast: 95 versions, 0.32.0 on 2026-10-26, first release 2026-06-04, MSRV 1.96 (README badge) |
| tuika [tuika] | own cell renderer; optional `ratatui` wrapper feature | views rebuilt each frame, host-owned `*State`, Runner/AsyncRunner | 30+ (Markdown, CodeBlock, Diff, Form, Slider, SelectList, TreeList, KeyedTable, CompletionPalette, Dialog, Toasts, KeyHints...) | Router + focus registry, declarative keymap | `Copy` Theme + StyleSheet | strong testing module | young (0.13.0, 2026-10-07, first release 2026-07-19) |
| ratcn [ratcn] | ratatui ^0.30.2 | shadcn-style copy-in components + Elm-like engine | 14 (Button, Input, TextArea, List, ScrollArea, Select, Tabs, Dialog, Toaster, BarChart, Tooltip, Checkbox, Cycle, Progress) | engine handles focus, keyboard, mouse, hover, layers | themeable (structure not described) | copy-fixture independence test | experimental (0.0.6, 2026-10-06) |
| FrankenTUI (`ftui`, 12+ crates) [ftui] | **not** based on ratatui (README treats it as an alternative) | Elm-style `Model` with `Cmd`s; render kernel with one-writer rule | ftui-widgets lists 67 modules incl. command_palette, file_picker, focus, help, modal, popover, notification_queue, textarea, tree, virtualized, validation_error | focus graph, modal stack, drag-and-drop | `StyleSheet`, themes | snapshots, PTY, harness | 0.9.0 (2026-10-03); README says API unstable, requires pinned nightly, non-goal "drop-in replacement"; some exotic experimental modules self-labelled; license badge reads "MIT with an OpenAI/Anthropic Rider" (**unverified**, check before reuse) |
| AppCUI-rs [appcui] | independent (own backends incl. crossterm) | desktop/window model | ~50 controls (listed in section 2) | key selector, command bar | color themes | not covered | active (0.5.6, 2026-10-24, 46 versions, first release 2025-06-07) |
| tui-react (Byron) [tui-react] | now `ratatui ^0.30` | "react-like paradigm" with mutable component state and render properties | List, Pane, Text (**unverified**) | | | | maintained (0.24.0, 2026-01-08; since 2019; 48k recent) |
| ratzilla [ratzilla] | ratatui ^0.30.1 | ratatui apps in the browser via WASM | re-exports ratatui; own `Hyperlink` widget; DOM backend in its example, Canvas backend linked in docs, WebGL2 credited in acknowledgements (crates.io shows a `beamterm-renderer ^1` dependency) | `on_key_event` callbacks | n/a | n/a | active (0.3.1, 2026-06-08, 239k recent; ratatui org) |
| dioxus-tui [dioxus-tui] | independent | Dioxus renderer | n/a | n/a | n/a | n/a | **abandoned in practice**: 0.5.0-alpha.0 on 2024-02-23, 266 recent downloads; Dioxus README does not list a terminal renderer |
| Others (metadata only) | | | | | | | `r3bl_tui` 0.7.8 (React/Elm, Flexbox, CSS, editor; 2026-01-23); `quadraui` 0.6.1 (one AppLogic on TUI/GTK4/macOS/Windows; 2026-10-26); `vtcode-ui` 0.180.5 ("design system, theme registry, and TUI framework" per crates.io; content not described in the repo README); `rnk` 0.19.3 (Ink/Bubbletea-like); `revue` 3.12.0 (Vue-like, CSS styling); `a3s-tui` 0.2.0 (TEA); `widgetui` 0.7.1 (stale, 2024-10-27); `ratatui-templates` (repo, not a crate): Hello World, Simple, Simple Async, Event Driven (+Async), Component templates via `cargo generate ratatui/templates` |

## 5. Crates examined

Columns: crate | version | last release (crates.io `updated_at`) | ratatui
compatibility (declared) | recent downloads | URLs (crates.io metadata, then
README/docs actually read). `cio` = `https://crates.io/api/v1/crates/<crate>`.
Compatibility notes come from
`https://crates.io/api/v1/crates/<crate>/<version>/dependencies`.

### 5.1 Ratatui core family

| Crate | Version | Last release | Ratatui compat | Recent dl | URLs |
| --- | --- | --- | --- | --- | --- |
| ratatui [ratatui] | 0.30.2 | 2026-06-19 | n/a (MSRV 1.86) | 23.8M | cio/ratatui ; https://github.com/ratatui/ratatui ; https://ratatui.rs/highlights/v030/ |
| ratatui-core | 0.1.2 | 2026-06-19 | n/a | 15.7M | cio/ratatui-core |
| ratatui-widgets [ratatui-widgets] | 0.3.2 | 2026-06-19 | needs `ratatui-core ^0.1.2` | 15.2M | cio/ratatui-widgets ; https://docs.rs/ratatui-widgets/latest/ratatui_widgets/ |
| ratatui-macros | 0.7.2 | 2026-06-19 | n/a | 11.8M | cio/ratatui-macros |
| ratatui-crossterm / -termion / -termwiz / -termina | 0.1.2 / 0.1.2 / 0.1.2 / 0.1.0 | 2026-06-19 | n/a | 15.1M / 1.87M / 11.9M / 9.5M | cio/ratatui-crossterm etc. (via search listing https://crates.io/api/v1/crates?q=ratatui&sort=recent-downloads&per_page=50) |
| crossterm | 0.29.0 | 2025-04-05 | n/a | 53.8M | same search listing |
| ratatui templates (repo) | n/a | n/a | n/a | n/a | https://raw.githubusercontent.com/ratatui/templates/main/README.md |
| tui (original tui-rs) | 0.19.0 | 2022-08-14 | superseded by ratatui | 376k | search listing (https://crates.io/api/v1/crates?q=tui%20widget&sort=recent-downloads&per_page=50) |

### 5.2 Second-party widgets (ratatui org, `ratatui/tui-widgets`)

| Crate | Version | Last release | Ratatui compat | Recent dl | URLs |
| --- | --- | --- | --- | --- | --- |
| tui-popup [tui-popup] | 0.7.7 | 2026-09-24 | `ratatui-core ^0.1`, `ratatui-widgets ^0.3` | 858k | cio/tui-popup ; https://raw.githubusercontent.com/ratatui/tui-widgets/main/tui-popup/README.md |
| tui-scrollview [tui-scrollview] | 0.6.8 | 2026-09-24 | core ^0.1, widgets ^0.3 | 103k | cio/tui-scrollview ; .../tui-scrollview/README.md |
| tui-scrollbar [tui-scrollbar] | 0.2.8 | 2026-09-24 | (not checked) | 530k | search listing ; .../tui-scrollbar/README.md |
| tui-big-text [tui-big-text] | 0.8.10 | 2026-09-24 | `ratatui ^0.30` | 108k | cio/tui-big-text ; .../tui-big-text/README.md |
| tui-prompts [tui-prompts] | 0.6.8 | 2026-09-24 | core ^0.1, widgets ^0.3, crossterm ^0.29 | 29k | cio/tui-prompts ; .../tui-prompts/README.md |
| tui-bar-graph, tui-box-text, tui-cards, tui-equalizer, tui-qrcode | n/a | n/a | n/a | n/a | listed only: https://raw.githubusercontent.com/ratatui/tui-widgets/main/README.md |
| ratatui-textarea [ratatui-textarea] | 0.9.3 | 2026-10-05 | core ^0.1.1, widgets ^0.3.1 (backends optional) | 636k | cio/ratatui-textarea ; https://raw.githubusercontent.com/ratatui/ratatui-textarea/main/README.md ; https://github.com/ratatui/ratatui-textarea (409 commits, 4 open issues, "fork of tui-textarea, maintained independently") |
| ratatui-image [ratatui-image] | 12.0.0-rc.3 (stable 11.1.0, 2026-09-17) | 2026-10-18 | rc: `ratatui ^0.30.1` | 609k | cio/ratatui-image ; https://raw.githubusercontent.com/ratatui/ratatui-image/master/README.md |
| tachyonfx [tachyonfx] | 0.25.2 | 2026-09-06 | core ^0.1.2 (ratatui dev) | 250k | cio/tachyonfx ; https://docs.rs/tachyonfx/latest/tachyonfx/ ; https://raw.githubusercontent.com/ratatui/tachyonfx/main/README.md |
| ansi-to-tui [ansi-to-tui] | 8.0.1 | 2026-01-10 | core ^0.1.0 | 3.42M | cio/ansi-to-tui |
| ratzilla [ratzilla] | 0.3.1 | 2026-06-08 | `ratatui ^0.30.1` | 239k | cio/ratzilla ; https://raw.githubusercontent.com/ratatui/ratzilla/main/README.md |

### 5.3 Third-party widgets

| Crate | Version | Last release | Ratatui compat | Recent dl | URLs |
| --- | --- | --- | --- | --- | --- |
| tui-input [tui-input] | 0.15.5 | 2026-09-26 | `ratatui ^0.30.2` optional; crossterm ^0.29, termion ^4 optional | 609k | cio/tui-input ; https://raw.githubusercontent.com/sayanarijit/tui-input/main/README.md |
| tui-textarea [tui-textarea] | 0.7.0 | **2024-10-22** | `ratatui ^0.29.0` optional, `tui ^0.19` optional | 707k | cio/tui-textarea ; https://github.com/rhysd/tui-textarea (no archive banner; 36 issues, 15 PRs) |
| tui-textarea-2 [tui-textarea-2] | 0.13.2 | 2026-08-23 | core ^0.1 / widgets ^0.3 optional; ratatui ^0.30 dev | 122k | cio/tui-textarea-2 |
| edtui [edtui] | 0.11.7 | 2026-08-16 | `ratatui ^0.30` dev; core ^0.1, widgets ^0.3, crossterm ^0.29 | 135k | cio/edtui ; https://raw.githubusercontent.com/preiter93/edtui/main/README.md |
| rat-text [rat-text] | 3.1.0 | 2026-03-08 | rat-* family on ratatui-core | 30k | https://docs.rs/rat-text/latest/rat_text/ |
| tui-tree-widget [tui-tree-widget] | 0.24.1 | 2026-08-09 | core ^0.1, widgets ^0.3 | 1.09M | cio/tui-tree-widget ; https://docs.rs/tui-tree-widget/latest/tui_tree_widget/struct.TreeState.html |
| tui-widget-list [tui-widget-list] | 0.15.3 | 2026-07-18 | core ^0.1, widgets ^0.3 | 157k | search listing ; https://raw.githubusercontent.com/preiter93/tui-widget-list/main/README.md |
| tui-logger [tui-logger] | 0.18.3 | 2026-07-04 | `ratatui ^0.30` | 389k | cio/tui-logger ; https://raw.githubusercontent.com/gin66/tui-logger/master/README.md |
| tui-term [tui-term] | 0.3.4 | 2026-04-07 | core ^0.1, widgets ^0.3 ; vt100 ^0.16 | 381k | cio/tui-term |
| tui-markdown [tui-markdown] | 0.3.12 | 2026-10-19 | core ^0.1 ; pulldown-cmark ^0.13 ; syntect ^5 optional | 197k | cio/tui-markdown ; https://raw.githubusercontent.com/joshka/tui-markdown/main/tui-markdown/README.md |
| ratatui-markdown [ratatui-markdown] | 0.3.6 | 2026-05-21 | (not checked) | 6.6k | cio/ratatui-markdown |
| tui-syntax-highlight [tui-syntax-highlight] | 0.2.0 | 2025-12-26 | (not checked) | 8.8k | cio/tui-syntax-highlight |
| syntect-tui [syntect-tui] | 3.0.6 | 2025-05-09 | (not checked) | 21k | search listing (https://crates.io/api/v1/crates?q=ratatui%20syntax%20highlight&sort=recent-downloads&per_page=20) |
| throbber-widgets-tui [throbber-widgets-tui] | 0.11.1 | 2026-06-19 | `ratatui ^0.30` | 264k | cio/throbber-widgets-tui ; https://raw.githubusercontent.com/arkbig/throbber-widgets-tui/main/README.md |
| tui-spinner [tui-spinner] | 0.5.0 | 2026-10-14 | (not checked) | 9k total | cio/tui-spinner |
| tui-skeleton [tui-skeleton] | 0.3.0 | 2026-03-31 | (not checked) | 1.4k | cio/tui-skeleton |
| tui-slider [tui-slider] | 0.4.6 | 2026-10-26 | `ratatui ^0.30` | 7.3k | cio/tui-slider ; https://raw.githubusercontent.com/sorinirimies/tui-slider/main/README.md |
| tui-checkbox [tui-checkbox] | 0.5.1 | 2026-10-14 | `ratatui ^0.30`; tui-nuances optional | 4.7k | cio/tui-checkbox ; https://raw.githubusercontent.com/sorinirimies/tui-checkbox/main/README.md |
| tui-piechart [tui-piechart] | 1.4.1 | 2026-10-26 | (not checked) | 14k | cio/tui-piechart |
| malevich [malevich] | 1.24.1 | 2026-10-03 | (not checked) | 2.8k total | cio/malevich |
| tui-dialog [tui-dialog] | 0.5.0 | 2026-01-17 | (not checked) | 164 | cio/tui-dialog |
| tui-overlay [tui-overlay] | 0.1.2 | 2026-04-07 | (not checked) | 1.6k | cio/tui-overlay |
| ratatui-comfy-toaster [ratatui-comfy-toaster] | 0.6.2 | 2026-07-18 | `ratatui ^0.30` | 530 | cio/ratatui-comfy-toaster ; https://gitlab.com/comfyhome/crates/ratatui-comfy-toaster/-/raw/main/README.md |
| ratatui-toaster [ratatui-toaster] | 0.1.4 | 2026-06-28 | (not checked) | 5.2k | cio/ratatui-toaster |
| ratatui-comfy-tabs [ratatui-comfy-tabs] | 0.5.12 | 2026-07-04 | (not checked) | 406 | cio/ratatui-comfy-tabs |
| tui-tabs [tui-tabs] | 0.1.1 | 2026-03-23 | (not checked) | 2.1k | cio/tui-tabs |
| tui-menu [tui-menu] | 0.3.1 | 2025-12-30 | `ratatui ^0.30.0` dev, core/widgets ^0.1/^0.3 | 2.3k | cio/tui-menu ; https://github.com/shuoli84/tui-menu (repo blurb still says "tui-rs ecosystem"; 35 commits) |
| rat-menu [rat-menu] | 3.1.1 | 2026-03-08 | rat-* family | 4.6k | https://docs.rs/rat-menu/latest/rat_menu/ |
| ratatui-explorer [ratatui-explorer] | 0.3.0 | 2026-03-06 | `ratatui ^0.30` | 49k | cio/ratatui-explorer ; https://github.com/tatounee/ratatui-explorer |
| tui-file-explorer [tui-file-explorer] | 3.2.0 | 2026-10-25 | `ratatui ^0.30` ; crossterm ^0.29 | 5.2k | cio/tui-file-explorer ; https://github.com/sorinirimies/tui-file-explorer |
| ratatui-hypertile [ratatui-hypertile] | 0.4.2 | 2026-10-01 | `ratatui ^0.30` | 4.4k | cio/ratatui-hypertile ; https://raw.githubusercontent.com/nikolic-milos/ratatui-hypertile/main/README.md |
| ratatui-form [ratatui-form] | 0.1.1 | 2026-01-31 | (not checked) | 6.1k total | cio/ratatui-form ; https://raw.githubusercontent.com/DavidLiedle/ratatui-form/main/README.md |
| ratiform [ratiform] | 0.7.1 | 2026-10-11 | `ratatui ^0.30` | 262 | cio/ratiform ; https://raw.githubusercontent.com/marc0x71/ratiform/main/README.md |
| schemaui [schemaui] | 0.16.3 | 2026-10-19 | (not checked) | 51k | cio/schemaui ; https://github.com/YuniqueUnic/schemaui |
| jkconfig [jkconfig] | 0.2.5 | 2026-06-15 | (not checked) | 81k | cio/jkconfig |
| rataflow [rataflow] | 0.1.0 | 2026-08-18 | (not checked) | 32k | search listing (https://crates.io/api/v1/crates?q=ratatui&sort=recent-downloads&per_page=50); described as node-based UIs with edges, pan, zoom and mouse |
| ratagrid [ratagrid] | 0.3.1 | 2026-10-11 | (not checked) | 96 | cio/ratagrid |
| ratatui-widgettable [ratatui-widgettable] | 0.1.2 | 2026-09-27 | (not checked) | 37 | cio/ratatui-widgettable |
| ratatui-code-editor [ratatui-code-editor] | 0.0.6 | 2026-07-07 | (not checked) | 2.5k | cio/ratatui-code-editor |
| hyperrat [hyperrat] | 0.1.3 | 2026-06-28 | (not checked) | 14.7k total | cio/hyperrat |
| ratatui-garnish | 0.1.0 | 2025-10-14 | (not checked) | 743 total | cio/ratatui-garnish |
| tui-panel-select [tui-panel-select] | 0.2.0 | 2026-09-23 | (not checked) | 3.0k | cio/tui-panel-select |
| hjkl-clipboard [hjkl-clipboard] | 0.43.4 | 2026-10-21 | n/a | 6.0k | search listing (https://crates.io/api/v1/crates?q=tui%20osc52%20clipboard&sort=recent-downloads&per_page=20) |
| hjkl-picker-tui [hjkl-picker-tui] | 0.43.4 | 2026-10-21 | adapter for hjkl-picker | 1.7k | cio/hjkl-picker-tui |
| nucleo-picker [nucleo-picker] | 0.13.0-beta.7 (stable 0.12.2) | 2026-10-22 | not ratatui | 15k | search listing (https://crates.io/api/v1/crates?q=nucleo%20picker&sort=recent-downloads&per_page=20) |
| terminput | 0.5.15 | 2026-07-18 | backend-agnostic input | 663k | cio/terminput |
| crokey | 1.5.1 | 2026-10-17 | n/a | 2.4M | cio/crokey |

### 5.4 Kits, component libraries, frameworks

| Crate | Version | Last release | Ratatui compat | Recent dl | URLs |
| --- | --- | --- | --- | --- | --- |
| rat-widget [rat-widget] | 3.2.1 | 2026-03-09 | rat-* (core ^0.1, widgets ^0.3) | 4.5k | cio/rat-widget ; https://docs.rs/rat-widget/latest/rat_widget/ ; https://github.com/thscharler/rat-salsa |
| rat-salsa [rat-salsa] | 4.0.3 | 2026-03-08 | core ^0.1, widgets ^0.3, ratatui-crossterm ^0.1 ; rat-event ^2.1, rat-focus ^2.1, rat-theme4 ^4.5, rat-widget ^3.2 | 2.0k | deps endpoint ; https://raw.githubusercontent.com/thscharler/rat-salsa/master/rat-salsa/readme.md |
| rat-focus [rat-focus] | 2.1.1 | 2026-03-10 | rat-* | 37k | https://docs.rs/rat-focus/latest/rat_focus/ |
| rat-event [rat-event] | 2.1.0 | 2026-03-08 | `ratatui-core`, `ratatui-crossterm` | 38k | https://docs.rs/rat-event/latest/rat_event/ |
| rat-theme4 [rat-theme4] | 4.5.3 | 2026-03-08 | core, widgets | 648 | cio/rat-theme4 ; https://docs.rs/rat-theme4/latest/rat_theme4/ |
| tuirealm [tuirealm] | 4.1.0 | 2026-05-02 | `ratatui ^0.30` ; crossterm/termion/termwiz optional | 44k | cio/tuirealm ; https://docs.rs/tuirealm/latest/tuirealm/ ; https://raw.githubusercontent.com/veeso/tui-realm/main/README.md |
| tui-realm-stdlib [tui-realm-stdlib] | 4.1.0 | 2026-05-02 | via tuirealm ^4 | 32k | https://docs.rs/tui-realm-stdlib/latest/tui_realm_stdlib/components/index.html |
| tui-realm-treeview / -textarea [tui-realm-treeview] [tui-realm-textarea] | 4.1.0 / 4.1.0 | 2026-05-02 | via tuirealm | 3.1k / 9.9k | cio/tui-realm-treeview ; cio/tui-realm-textarea |
| cursive [cursive] | 0.21.1 | **2024-08-03** | independent | 508k | cio/cursive ; https://docs.rs/cursive/latest/cursive/views/index.html ; https://github.com/gyscos/cursive |
| iocraft [iocraft] | 0.9.3 | 2026-10-14 | independent | 55k | cio/iocraft ; https://raw.githubusercontent.com/ccbrown/iocraft/main/README.md ; https://docs.rs/iocraft/latest/iocraft/components/index.html |
| dioxus-tui [dioxus-tui] | 0.5.0-alpha.0 | **2024-02-23** | independent | 266 | cio/dioxus-tui ; https://raw.githubusercontent.com/DioxusLabs/dioxus/main/README.md |
| ftui + ftui-widgets, -a11y, etc. [ftui] [ftui-widgets] | 0.9.0 | 2026-10-03 | independent | 27k | cio/ftui ; https://docs.rs/ftui-widgets/latest/ftui_widgets/ ; https://docs.rs/ftui-a11y/latest/ftui_a11y/ ; https://raw.githubusercontent.com/Dicklesworthstone/frankentui/main/README.md |
| tui-lipan [tui-lipan] | 0.32.0 | 2026-10-26 | `ratatui ^0.30` | 5.4k | cio/tui-lipan ; https://raw.githubusercontent.com/tui-lipan/tui-lipan/main/README.md |
| tuika [tuika] | 0.13.0 | 2026-10-07 | optional ratatui feature; ratatui ^0.30.2 dev | 4.3k | cio/tuika ; https://raw.githubusercontent.com/everruns/tuika/main/README.md |
| ratcn [ratcn] | 0.0.6 | 2026-10-06 | `ratatui ^0.30.2` | 924 | cio/ratcn ; https://raw.githubusercontent.com/kristoferlund/ratcn/main/README.md |
| ratatui-kit [ratatui-kit] | 0.12.0 | 2026-10-11 | `ratatui ^0.30.1` | 2.3k | cio/ratatui-kit ; https://raw.githubusercontent.com/yexiyue/ratatui-kit/main/README.md |
| ratatui-interact [ratatui-interact] | 0.5.3 | 2026-04-02 | `ratatui ^0.30` | 46k | cio/ratatui-interact ; https://raw.githubusercontent.com/Brainwires/ratatui-interact/main/README.md |
| ratatui-cheese [ratatui-cheese] | 0.7.0 | 2026-04-28 | `ratatui ^0.30` | 13k | cio/ratatui-cheese ; https://raw.githubusercontent.com/shashanktomar/ratatui-cheese/main/README.md |
| ratkit [ratkit] | 0.2.18 | 2026-06-16 | (not checked) | 217 | cio/ratkit |
| tui-react [tui-react] | 0.24.0 | 2026-01-08 | `ratatui ^0.30` | 49k | cio/tui-react ; https://raw.githubusercontent.com/Byron/tui-crates/main/README.md (README still says it depends on `tui`; crates.io dependency data says ratatui ^0.30, so the README is stale) |
| appcui [appcui] | 0.5.6 | 2026-10-24 | independent | 6.8k | cio/appcui ; https://raw.githubusercontent.com/gdt050579/AppCUI-rs/main/README.md |
| quadraui | 0.6.1 | 2026-10-26 | (not checked) | 1.0k | cio/quadraui |
| vtcode-ui | 0.180.5 | 2026-10-22 | (not checked) | 4.4k | cio/vtcode-ui ; https://github.com/vinhnx/vtcode (README does not describe it) |
| tui-pantry [tui-pantry] | 0.4.0 | 2026-04-15 | `ratatui ^0.30` | 287 | cio/tui-pantry ; https://raw.githubusercontent.com/taho-inc/tui-pantry/main/README.md |

### 5.5 Theming and testing

| Crate | Version | Last release | Ratatui compat | Recent dl | URLs |
| --- | --- | --- | --- | --- | --- |
| opaline [opaline] | 0.4.2 | 2026-09-02 | `ratatui ^0.30` optional feature | 22.6k | cio/opaline ; https://raw.githubusercontent.com/hyperb1iss/opaline/main/README.md |
| tui-nuances [tui-nuances] | 0.1.10 | 2026-10-26 | `ratatui ^0.30` | 2.5k | cio/tui-nuances ; https://raw.githubusercontent.com/sorinirimies/tui-nuances/main/README.md |
| ratatui-themes [ratatui-themes] | 0.3.0 | 2026-08-02 | `ratatui ^0.30.2` | 9.9k | cio/ratatui-themes ; https://raw.githubusercontent.com/ricardodantas/ratatui-themes/main/README.md |
| ratatui-themekit [ratatui-themekit] | 0.6.1 | 2026-04-06 | `ratatui ^0.30` | 1.7k | cio/ratatui-themekit ; https://raw.githubusercontent.com/diegorodrigo90/ratatui-themekit/main/README.md |
| makeover-geometry [makeover-geometry] | 0.8.3 | 2026-10-11 | none (tokens only) | 1.9k | cio/makeover-geometry ; https://docs.rs/makeover-geometry/latest/makeover_geometry/ |
| catppuccin [catppuccin] | 2.8.0 | 2026-04-26 | `ratatui-core` feature | 45k | cio/catppuccin |
| termprofile [termprofile] | 0.2.4 | 2026-05-02 | n/a | 6.7k | cio/termprofile |
| ratatui-base16 / chromata | 0.3.7 (2024-12-21) / 1.0.0 (2026-03-31) | | | 551 / 473 | https://crates.io/api/v1/crates?q=base16%20ratatui&sort=recent-downloads&per_page=15 |
| insta [insta] | 1.49.0 | 2026-10-03 | n/a | 34.9M | cio/insta |
| termlens [termlens] | 0.11.7 | 2026-10-27 | n/a (PTY) | 7.1k | cio/termlens ; https://ratatui.rs/recipes/testing/snapshots/ |
| termwright [termwright] | 0.2.0 | 2026-02-02 | n/a (PTY) | ~1.7k total | cio/termwright |
| termproof [termproof] | 0.4.4 | 2026-10-26 | n/a (PTY) | 5.0k | cio/termproof |
| ratatui-testlib [ratatui-testlib] | 0.1.0 | 2025-12-01 | ratatui, Sixel, Bevy | 6.8k total | cio/ratatui-testlib |

Also seen by metadata or by name in another README only (not read, not
counted above): `rnk`, `revue`,
`a3s-tui`, `sectioned-picker`, `tui-rating`, `tui-chart` (the last two named
as tui-nuances consumers), `tui-shimmer`,
`aisling`, `boxmaid`, `ratatui-input-manager`, `nobubbles`, `clark`,
`charmed-bubbles`, `envision`, `mdstream`, `ratatui-sci-fi`, `resq-tui`,
`tb-tui-common`, `plurimus_*`, `term-wm`, `superlighttui`, `ratatui-wasm`,
`color-to-tui`, `coolor`, `ggsci-ratatui`, `webatui`, `egui-ratatui`, `mousefood`.
Source for the awesome-list entries:
https://raw.githubusercontent.com/ratatui/awesome-ratatui/main/README.md .

Distinct crates and projects examined with crates.io metadata: well over 100
(the tables above plus the metadata-only list); read in detail (README or
docs page): roughly 60.

## 6. Abandoned, stale, or risky

| Crate | Evidence | Consequence |
| --- | --- | --- |
| `tui` (tui-rs) | 0.19.0, 2022-08-14; superseded by ratatui | Do not use. |
| `tui-textarea` | 0.7.0 on 2024-10-22; declares `ratatui ^0.29.0`, so it will not resolve against ratatui 0.30; GitHub shows no archive banner; still 707k recent downloads | Replace with `ratatui-textarea` (ratatui org, 0.9.3, 2026-10-05) or `tui-textarea-2` (0.13.2, 2026-08-23). The repo's deferred `tui-textarea` adoption (CSP-430) should evaluate the successors instead. |
| `cursive` | 0.21.1 on 2024-08-03; repo has 191 open issues but no archive banner | Release-stale for ~2.2 years; not ratatui-based anyway. |
| `dioxus-tui` | 0.5.0-alpha.0 on 2024-02-23; 266 recent downloads | Effectively abandoned (the current Dioxus README does not list a terminal renderer). |
| `widgetui` | 0.7.1 on 2024-10-27 | Stale. |
| `ratatui-base16` | 0.3.7 on 2024-12-21 | Stale. |
| `nucleo` / `nucleo-matcher` | 0.5.0 on 2024-04-02 / 0.3.1 on 2024-02-20 (yet 698k / 2.1M recent downloads); forks exist (`atuin-nucleo`, `television-nucleo`, `ncp-*`) | No upstream release for about 2.5 years; widely used regardless, and several forks exist. |
| `tui-menu` | 0.3.1 on 2025-12-30; repo blurb still says "tui-rs ecosystem"; 35 commits | Works with 0.30 per dependency data, but low activity. |
| `syntect-tui` | 3.0.6 on 2025-05-09 | Probably fine (small glue), release-stale. |
| `ratatui-calendar` | 0.0.0 placeholder | Name squat; do not use. |
| FrankenTUI (`ftui*`) | pre-1.0; README says API unstable and requires nightly; unusual license badge | Treat as a source of ideas, not a dependency. |
| `rat-*` crates | active but ~2k to 5k recent downloads for the top crates; single maintainer | Matches the repo's earlier no-go on adoption; usable as design reference. |

## 7. Surprises and observations for the gap analysis

1. **The "official" text-area moved.** `ratatui-textarea` is the maintained
   continuation (ratatui org); the original `tui-textarea` is two ratatui
   minors behind. This affects any plan that referenced `tui-textarea`.
2. **Ratatui's built-in tabs, list, table, scrollbar and gauge carry no event
   handling at all.** The only selection model is single-row/cell; the 0.30
   `List` still has no multi-select.
3. **Hit testing is the universal mouse convention**, but each crate has its
   own type (`rendered_at`, `hit_test`, `ClickRegionRegistry`, `ScrollCommand`).
   No crate offers a shared mouse-region registry independent of its widgets
   except ratatui-interact and FrankenTUI's hit-region constants.
4. **Declarative key tables that also generate help** exist (tuika, schemaui,
   tui-file-explorer) but are rare and young; the Conspectus approach
   (CSP-468: one table feeding dispatch, overlays and help) matches the best
   of what is published rather than trailing it.
5. **No adopted command palette, wizard, breadcrumb, accordion, date-picker
   or autocomplete crate exists as a standalone**; they live only inside
   large kits (ratatui-interact, tui-lipan, tuika, ftui, AppCUI, rat-widget).
6. **Theme systems are color-only** except makeover-geometry. Semantic slot
   sets converge on: foreground/background, muted, accent/primary, border
   (+ focused), selection, success/warning/error/info, and sometimes diff
   added/removed/context. `NO_COLOR` and color-depth downgrade are handled by
   only two crates (tui-nuances, ratatui-themekit).
7. **Snapshot tests cannot assert color through `TestBackend` + insta**
   (ratatui issue #1402); the ecosystem workarounds are PTY harnesses
   (`termlens`, `termwright`), headless ANSI dumps (`tui-pantry dump`), and
   kit-specific capture formats (tui-lipan PNG/JSON/Markdown). Conspectus's
   `tui --snapshot` with ANSI preserved is in that same family.
8. **Accessibility is essentially absent** from the Rust TUI ecosystem;
   only `ftui-a11y` exists and its screen-reader bridge is unimplemented.
9. **Independence-by-copy (ratcn) and independence-by-fixture** is a real
   alternative to workspace-crate enforcement; ratcn's `copy-fixture` crate
   is a testable version of it.
10. **Release cadence is high and downloads are small** for the new design
    kits (tui-lipan: 95 versions in about five months). Version churn makes
    them poor dependencies but good sources of vocabulary: control names,
    state shapes, outcome enums (`Selected`/`Dismissed`, `Submitted`/
    `Cancelled`/`Working`, `ToastInteraction`), and test helpers.
11. **tachyonfx composes as a post-render pass over any widget buffer**, so
    animations can be added later without changing widget code, which makes
    "animation" a low-risk gap to defer.

## 8. Unverified or not examined

- Behavior of widgets whose READMEs were not fetched or came back thin:
  tui-term key forwarding, ratatui-code-editor, ratagrid sorting API,
  ratatui-widgettable, tui-qrcode, tui-equalizer, tui-cards, tui-bar-graph,
  tui-piechart, malevich, boxmaid, rataflow, tui-overlay, tui-tabs,
  ratatui-comfy-tabs, hyperrat internals, jkconfig, ratkit.
- Chart `GraphType` variants, `Canvas` options, `Gauge`/`LineGauge`
  method-level API, `BarChart` direction options (module index pages only).
- The ratatui "Popups" recipe, the Component/TEA/Flux architecture pages,
  and `ratatui-explorer`'s use of unstable `render_widget_ref` feature
  enabling.
- Whether `tui-textarea` 0.7.0 builds against ratatui 0.30 (declared
  requirement is `^0.29.0`, so Cargo would not unify them; not tested).
- `rat-focus` containers, mouse-click focus and modal focus (not on the
  fetched page); `rat-menu` mnemonics and popup placement.
- FrankenTUI license text and quality; claims quoted from its own README are
  the project's own and were not independently checked.
- `ratatui-dropdown`, `tui-themes`, `tui-file-dialog`: queried and **not
  found** (404 or absent in search results).
- Search tool: the web search tool was unavailable (API error), so discovery
  relied on the crates.io search API, the awesome-ratatui list, and direct
  fetches. Crates with unusual names that none of those surfaced may exist.
- Download counts are crates.io counters and include transitive pulls
  (for example `tui-textarea` and `tui-scrollbar` appear popular partly
  because other crates depend on them).
