# ADR 0165: Text Entry Has One Line Editor And One Text Field

## Status

Accepted. Narrows [ADR 0030](0030-tui-text-input-primitive.md) (text-input
primitive): the line-edit engine moves out of `src/tui/widgets/input.rs` into
the framework's `text_edit` module as `LineEdit`, still wrapping `tui-input`,
and every text field is drawn by the framework's `TextField`. The modal prompt
and its `Enter`/`Esc`/`Ctrl-C`/`Tab` contract stay in the host as
`TextPromptState` until `TextPrompt<M>` takes them. Applies house rules 13, 14,
and 15 of [ADR 0151](0151-tui-framework-control-set-and-house-rules.md) and
extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`text_edit` module. Carries out [ADR 0152](0152-tui-framework-dependency-ownership.md)'s
note that `LineEdit` maps the modifier `tui-input` binds to Meta.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/03-text-entry-and-forms.md`, sections 1,
2, 4, and 7; `05-search-help-viewers.md`, section 2.10) counted eleven text
fields behind two edit engines and four renderers:

- **Two engines.** Ten fields edited through `TextInputState` (`tui-input`);
  the transcript viewer's and the pane viewer's find prompts used a hand-rolled
  `FindPrompt` with its own key subset: no `Ctrl-W`, `Ctrl-K`, `Ctrl-Y`,
  `Ctrl-A`, or `Ctrl-E`, and `Ctrl-U` deleting only before the cursor.
- **Three windowing algorithms.** The prompt's `visible_window`, the forms'
  `window`, and the viewers' `draw_find_prompt` each kept the cursor in view
  their own way; only the last measured terminal cells. A CJK or emoji value
  in a prompt or a form put the caret on the wrong cell, ran past the field,
  and pushed what followed it (a path's validity mark, the pin edit form's tag
  column) out of place.
- **No caret in the query lines.** Quick select, content search, and the
  picker drew `/` and the query, clipped at the frame, with no cursor cell, so
  the typing position of a long or edited query could not be seen.
- **Copy and compare.** The path field, the content search, and the pin edit
  form copied the value before each key and compared it after to learn
  whether it changed; the picker refiltered on every key instead.
- **Unreachable arms.** The new mux, mux launch, and pin edit forms matched
  `InputOutcome::Confirm` (and `Cancel`) from the line editor although each
  resolves `Enter` and `Esc` in its own table first.
- **Meta, not Alt.** `tui-input`'s crossterm backend binds the word keys
  (`b`, `f`, `d`, `Backspace`) to Meta, which a terminal reports as Alt, so
  the prompt table carried its own `Alt-b`/`Alt-f` rows and the find prompts
  their own copy.

## Decision

1. **`tuikit::text_edit` owns editing one line and drawing it.** Its items:
   `LineEdit`, `Edit`, `single_line_paste`, and `TextField`, with `EditKey`
   and `LINE_EDIT_KEYS` (kept with the standard tables in `keys` and
   re-exported). The host keeps what a field is for: its label, what `Enter`
   and `Esc` do, what a change triggers, and whether a paste reaches it.

2. **`LineEdit` is the only edit engine.** It wraps `tui-input`'s `Input`
   and drives it with `InputRequest`s, so `tui-input` still owns graphemes,
   word boundaries, and the kill buffer. Keys resolve through
   `LINE_EDIT_KEYS`, readline's keys with Meta mapped to Alt: `←`/`→`
   (`Ctrl-B`/`Ctrl-F`), `Home`/`End` (`Ctrl-A`/`Ctrl-E`), `Backspace`
   (`Ctrl-H`), `Delete`, word motion on `Alt-b`/`Alt-f`, `Ctrl-←`/`Ctrl-→`,
   and `Alt-←`/`Alt-→`, `Ctrl-W` and `Alt-Backspace` (word before), `Alt-d`
   and `Ctrl-Delete` (word after), `Ctrl-U` (the line), `Ctrl-K` (to the
   end), and `Ctrl-Y` (yank). A character typed alone, with Shift, or with
   Control and Alt (AltGr) inserts. The editor has no `Enter`, `Esc`, or
   `Tab`.

3. **An edit says what it did.** `handle(key)` returns `Edit { changed,
   consumed }`: `consumed` when the key is an editing key or types,
   `changed` when the text changed. `Ctrl-K` at the end, which the engine
   reports as a change, changes nothing. `EditKey::of(key)` names the edit a
   key asks for, so a host that turns keys into messages carries the edit as
   a value (the viewers' `ViewerFindMsg::Edit`).

4. **Paste is the framework's rule.** `single_line_paste` is the flattening
   ADR 0151's rule 15 asked for (line breaks at the ends dropped, each inner
   run one space, tab a space, other control characters dropped), and
   `LineEdit::paste` types it. The host still types a bracketed paste one key
   at a time into whichever field has the keys, so a field completes and
   refilters as it does for typing.

5. **One windowing rule, in cells.** `TextField` draws a `LineEdit` in one
   row. The window starts at the line's start while the text before the cursor
   and the cell under it fit; otherwise it starts at the first grapheme that
   lets them fit, so the cursor rides the right edge. After the cursor it
   shows what fits; a wide character never straddles the edge, and one wider
   than the field draws as `…`. This is the rule all three old algorithms
   follow for ASCII, so ASCII frames do not move.

6. **One look.** The caret is the cell under the cursor in `field_cursor`
   while the field is focused, and none when it is not. Options: `<` and `>`
   marks in 2-cell slots at each end (`accent`, bold, over the field's
   style; a field under 5 cells has no slots), a prefix that never scrolls
   (the `/` of a query), a placeholder for an empty value (the forms' `-`),
   and a fill to the full width. A whitespace-only value is text, not empty.

7. **The host moves onto it.** The prompts keep their contract in
   `TextPromptState` over a `LineEdit`; the forms, the path field, the launch
   spec, the search query, and the picker hold `LineEdit`s and use `changed`;
   the forms lose their `Confirm` arms; the find prompts are `LineEdit`s and
   `VIEWER_FIND_KEYS` keeps only `Ctrl-C`, `Enter`, and `Esc`;
   `TEXT_INPUT_KEYS` loses its word rows. The form rows, the query lines, the
   prompt's row, and the find footer draw through `TextField`.

## Consequences

- Wide characters edit, window, and keep their columns in every field. The
  search and picker query lines draw a caret and keep it in view. The
  prompts' and find footers' caret takes `field_cursor` (black on white by
  default) instead of reverse video, the look the forms had.
- The find prompts edit like every other field: they gain the readline keys,
  and `Ctrl-U` deletes the whole line, not only what precedes the cursor.
  Word motion there follows the shared word rule (Unicode word boundaries),
  which steps between ideographs.
- Every field gains `Alt-←`/`Alt-→` and `Alt-d`, and a stray Shift on
  `←`, `→`, `Home`, `End`, `Backspace`, or `Delete` no longer swallows the
  key.
- An unfocused form field draws the cell under its hidden cursor in its own
  style (the `-` of an empty one included), where that cell was unstyled.
- The host no longer depends on `tui-input` directly; the framework does.
- `TextPrompt<M>`, `PathField`, and `Form<M>` build on these pieces.

## Alternatives Considered

- **Keep `tui-input`'s crossterm handler and add Alt rows in each host
  table.** Least code, but the editing keys would be spread over tables that
  mean other things, with nothing to document as the field's keys, and a
  message-driven host (the viewers) could not carry an edit as a value.
- **Keep the find prompts' own editor.** The viewers are below the
  framework now, so the reason for a separate engine (ADR 0052's boundary)
  is gone; two engines would keep two key sets.
- **A stateful window that scrolls by minimal motion.** Steadier when the
  cursor moves left inside a long value, but the renderer takes the editor by
  reference and keeps nothing; the stateless rule is what every old renderer
  did for ASCII, so frames do not move. It can be added behind the same API.
- **Show the terminal's own cursor** (`Frame::set_cursor_position`). It
  blinks and reads as a text cursor, but only one is drawn per frame, it is
  invisible in snapshots and goldens, and a form shows several fields. The
  styled cell stays; the position is a seam.
- **Keep reverse video for the caret.** The framework draws no raw
  modifiers (house rule 13), and `field_cursor` already is the forms' caret.

## Open Questions Answered

- *Do the query lines draw a caret?* Yes, the same `field_cursor` cell as
  every field.
- *Does `Ctrl-U` in a find prompt keep deleting only before the cursor?* No:
  it deletes the line, as in every field.
- *Where do the editing keys live?* In one table, `LINE_EDIT_KEYS`, with the
  other standard tables, consulted after an overlay's own table.
