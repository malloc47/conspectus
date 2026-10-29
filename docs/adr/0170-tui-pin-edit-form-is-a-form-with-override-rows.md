# ADR 0170: The Pin Edit Form Is A `Form` With Override Rows

## Status

Accepted. Amends [ADR 0168](0168-tui-forms-one-ring-submit-from-any-field-fixed-footer.md):
`tuikit::form` gains the override row, an intro paragraph, and a band of
detail lines, and the pin edit form, the last form that laid out its own
rows, is a `Form`. Finishes what
[ADR 0169](0169-tui-launch-form-is-a-form-and-one-grammar-splits-every-argv.md)
left for it (its own launch-option rule). The pin slot semantics of
[ADR 0141](0141-pin-slots.md) are unchanged.

## Context

The pin edit form (**Edit…** in a pin's menu) lists every field a pin can
change. ADR 0141 gives each field a value from the first layer that has
one (overrides, the live graph, history, the snapshot, defaults), and the
form shows that value, where it comes from, and lets the operator set an
override or drop one with `Ctrl-R` so the field follows the entity again.
After ADRs 0168 and 0169 it drew its rows through the framework's row kit
and body but kept its own state, ring, keys, and grid. The inventory for
the framework extraction
(`docs/plans/tui-framework/inventory/03-text-entry-and-forms.md`, section
2.7 and items D8, D16) found:

- **A generic mechanism in a host file.** Each row carried its value when
  the form opened, whether it held an override, the value it would have
  without one and where that comes from, and whether `Ctrl-R` was
  pressed; a right-hand column showed a word for that state. Nothing in
  it is about pins except the words.
- **`Ctrl-R` means two things.** Reset in this form, recall an earlier
  query in the content search. A global binding cannot serve both.
- **Its own grid and refusals.** A label column as wide as the raw field
  names (`worktree_branch`, `resume_into`), the error line among the
  detail lines rather than above the keys, and a refused save that left
  the focus where `Enter` was pressed.
- **Two rename UIs.** **Rename…** opens a prompt (ADR 0166) that refuses
  only a blank name and lets the store refuse the rest; the form's first
  row renames too, checking the name against the pin id rules and the
  other pins before it saves. Both end in the same store write.

## Decision

1. **`OverrideRow` decorates a field.** `Field::with_override` gives a
   text, path, or choice field an inherited value, a source word (where
   that value comes from, in the host's words), whether the field holds a
   value of its own as the form opens, and an optional hint closure. The
   field's `Standing` is `Follows`, `Kept`, `Changed` (a submit keeps the
   new value), or `Reset` (a submit drops the field's own value). Editing
   changes it; the reset key puts the inherited value back, which is a
   `Reset` only when there was a value of its own to drop and `Follows`
   otherwise; typing after a reset makes the value the field's own again.
   `Values::standing` gives `on_submit` what to keep or drop. A toggle,
   check group, one-of-N row, or read-only line cannot follow a value.

2. **The reset key is the form's own.** `FormKey::Inherit` is in no
   standard table; a form with override rows binds a key to it in its own
   table (the pin edit form binds `Ctrl-R`, with `follow again` in its
   footer). It acts on the focused override row and nothing else, never
   reaches a field's text, and clears the error line as an edit does when
   it changes something. Another overlay is free to give the same chord
   another meaning.

3. **One source column.** In a form with override rows, every text, path,
   and choice row ends in one right-aligned column as wide as the longest
   word any row may show: the source word while the field follows, `kept`,
   or, in `warning` while a change is pending, `will keep` or `will
   follow`. A row that is not an override row leaves it blank, so every
   value ends in one place and the words never move with the focus.

4. **Explanations stay in view.** `Form::intro` is a paragraph fixed above
   the rows; `Form::details(n)` is a band of `n` lines fixed under them
   that shows the focused field's hints (its override row's hint, then
   its own `Field::hint` lines), drawn at full width so only its words
   change as the focus moves. Both keep the form's height steady; on a
   short terminal they give way after the focused row, the error line, and
   the footer.

5. **The pin edit form is a `Form<Msg>`.** `PinEditForm` holds it and
   keeps the launch options and a carried argv in step after each key
   with the launch form's helpers (`launch_argv`), as ADR 0169's host
   does. Its fields, in order: a note naming the pin, its kind, and its
   status; `pin name` (the rename, checked with the pin id rules and
   against the other pins); for a live mux pin, a note that the rename
   renames the tmux session; then `label` and the kind's overridable
   fields as override rows (directories as path fields, the mux harness
   as a choice that takes a typed value, the mux argv checked as `pin edit
   --set` reads it, and the launch options as a check group under the
   harness). Labels take the shared vocabulary of ADR 0168
   (`worktree_branch` reads `branch`, `resume_into` reads `resume in`).
   The intro explains following; two detail lines explain the focused
   row. Conspectus keeps the field map by pin kind, the labels, the layer
   words (`running`, `last seen`, `saved`, `default`, `unset`), the hints,
   the rename checks, and the save: `on_submit` builds the same
   `PinEditRequest` (a rename, overrides to set, overrides to drop) and
   the runtime writes it as `StoreOp::SavePin`.

6. **A field without an override follows the value it has now.** Its
   inherited value is its value in effect and its source word that
   value's layer, which may come from another field's override (the
   default argv of an overridden harness). A field with an override
   follows its value without the pin's overrides, as before.

7. **Both rename UIs stay.** The prompt is the one-key rename `r` gives
   every renameable row (sessions, muxes, pins), and the form row renames
   in the same save as the overrides; removing either would lose one of
   those gestures, and both already end in the same store write.
   Unifying them is not a small change, so the prompt keeps its checks
   (blank is refused in the prompt; the store refuses the rest) until
   backlog `CSP-778` gives it the form's.

## Consequences

- **Visible changes** (the story's final summary and the changelog list
  them): the labels shorten (`branch`, `resume in`), so the label column
  narrows and values and harness chips get more room; the note naming the
  pin moves under the intro and scrolls with the rows; a refused save
  puts its reason on the error line directly above the keys (it sat
  under the detail lines) and moves the focus to the field it is about;
  `Ctrl-R` on the pin name no longer puts back the name the form opened
  with; resetting a field that had no override shows its source word
  again instead of `will follow`; a kept value with nothing to follow
  says `Ctrl-R drops it; there is nothing to follow yet`; and on a very
  short terminal the whole footer shows while the detail lines give way.
- The host's form row helpers (`widgets/form.rs`) had one consumer and
  are gone; their tests of the theme reaching the rows draw the
  framework's rows directly.
- A new form whose fields can follow a value is a `Form` with
  `with_override` on those fields and a reset key in its table.

## Alternatives Considered

- **A field kind for overrides** (`FieldKind::Override(Box<FieldKind>)`).
  It would nest kinds and make every match over `FieldKind` handle the
  wrapper; a decorator on `Field` keeps the kinds flat and lets any text,
  path, or choice field follow a value.
- **Bind the reset key in `FORM_KEYS`.** Every form would then claim
  `Ctrl-R`, including those with nothing to reset, and the footer would
  name a key that does nothing. Rejected; the binding belongs to the
  form that needs it.
- **Framework-chosen words only, or host words only.** The four words
  are plain English and every form with override rows needs them, so the
  framework names `kept`, `will keep`, and `will follow`; the source word
  is the host's because only the host knows where a value comes from.
- **Keep the summary line fixed above the rows.** A fixed band of host
  rows would need a second kind of field; the intro is the fixed band,
  and the summary repeats the name the first row shows.
- **Show the detail lines before the footer on a short terminal.** The
  keys and a refusal matter more than an explanation; ADR 0168's order
  stands.
- **One rename UI.** Dropping the prompt would make `r` on a pin open a
  form of up to nine rows for one field, unlike every other rename;
  dropping the row would make renaming and editing two saves. Rejected
  for now; giving the prompt the form's checks is backlog `CSP-778`.

## Open Questions Answered

- *Is the override row generic?* Yes: an inherited value, a source word,
  a hint, and a reset key the form's own table binds.
- *Is `Ctrl-R` global?* No; it is a row of the pin edit form's table.
- *Do both rename UIs stay?* Yes, for the reasons in decision 7.
