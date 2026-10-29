# ADR 0168: Forms Move On One Ring, Submit From Any Field, And Keep Their Keys In View

## Status

Accepted. Carries out the `Form<M>` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying its
house rules 6, 7, 8, 13, and 17 to the multi-field forms, and builds on the
line editor, the text prompt's error line, and the path field of
[ADR 0165](0165-tui-text-entry-one-line-editor-and-one-text-field.md),
[ADR 0166](0166-tui-text-prompts-one-overlay-that-explains-its-refusals.md),
and [ADR 0167](0167-tui-path-fields-complete-behind-a-source-and-expand-tilde-on-commit.md).
Extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `form` module. Amends
[ADR 0154](0154-tui-glyph-marks-vetted-table-and-scan.md): the marks gain the
derived-line corner `└`, and the framework names the option boxes built
from vetted glyphs (`[x]`, `[ ]`, `(•)`, `( )`). Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`Space` in `FORM_OPTION_KEYS` is its own action, `FormKey::Toggle`, under
the same `change` verb.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/03-text-entry-and-forms.md`, sections
2.3 to 2.7 and 3 to 7) found three forms (the new mux form, the mux launch
form, and the pin edit form) sharing a row kit but not a behavior:

- **Two meanings of `Enter`.** The new mux form moved from the name field
  to the cwd field on `Enter` and created the session only from the cwd
  field; the launch and pin edit forms submitted from any row.
- **Two rings.** The launch form moved between rows on `↑`/`↓` and between
  stops on `Tab`/`S-Tab`, with a socket checkbox that only `Tab` reached;
  the other two had one ring.
- **Three ways to refuse.** The new mux form posted a toast (an interim
  that replaced a silent refusal); the launch form wrote its reason after
  a blank line under the rows, where on a short terminal it scrolled away
  with the footer; the pin edit form wrote it among its fixed bottom lines.
- **Footers that scroll away.** The new mux and launch forms drew their
  footer as the last lines of a body that scrolled whole, so on a short
  terminal the keys scrolled off; only the pin edit form kept them.
- **Radio rows that look like checkboxes.** The launch form's one-of-three
  pin row drew `[x] don't pin [ ] launch, then pin`, exactly as a row of
  checkboxes, and the launch form hand-wrote the worktree toggle's box.
- **Two label vocabularies.** The launch form's labels were `mux name`,
  `mux socket`, and `launch argv` where the new mux form said `name` and
  the house rule says the short form.
- **Dead and copied helpers.** `warning_row` had no caller, and
  `options_row` and `choice_row` were the same row with different inputs.

## Decision

1. **`tuikit::form` owns the form.** Its items: `Form<M>`, `Field`,
   `FieldKind`, `Choice`, `Toggle`, `CheckGroup`, `OneOf`, `ReadOnly`,
   `Tone`, `Values`, `Submit<M>`, `FormBody`, `FORM_SIZE`, and the row kit
   (`Cols` and the row functions). A form is an ordered list of fields, each
   with an id, a label, and a kind: text, a path (`PathField`), a choice of
   named options shown as chips (optionally taking a typed value of its
   own, which the row marks `[custom]`), a toggle, a check group, a one-of-N
   row, or a read-only line in a tone (a note, derived output, a warning).
   The host keeps the fields, their seeds and labels, the key table, the
   checks' reasons, and what a submit means.

2. **One ring, one stop per field.** `Tab`, `↓`, and `Ctrl-N` go to the next
   stop and `S-Tab`, `↑`, and `Ctrl-P` to the previous, wrapping. A
   read-only line is never a stop. `Tab` on a path field completes first
   and moves on only when there is nothing to complete. A field may be
   shown only while a condition over the form's values holds
   (`visible_if`); a hidden field has no row, no stop, and no check, and
   when the focused field is hidden the focus moves to the stop after it.

3. **`Enter` submits from any field (house rule 6).** Each shown field's
   check (`validate`, over the text as typed) runs top to bottom; the first
   that fails puts its reason on the error line and takes the focus. When
   all pass, the host's `on_submit` gets the values and answers with a
   `Submit<M>`: `Commit(M)`, `Reject(String)`, or `Close` (a save that
   changes nothing). `on_submit` is shared behind an `Arc` and must be
   `Send + Sync`, as the prompt's `on_confirm` is, so a form clones and the
   host's enum of overlays stays `Clone`.

4. **One error line, in the footer slot (house rule 7).** The reason sits on
   one line directly above the footer, in `error`, cut with `…`, and the
   form grows a row while it shows. The next key that changes a value
   (typing, stepping an option, flipping a toggle, completing a path)
   clears it; moving does not.

5. **A fixed footer (house rule 8).** The footer is generated from the
   form's key table and broken only between hints. The body
   (`FormBody`: a fixed top band, the rows, and fixed notes, error line,
   and footer) scrolls only its rows, keeping the focused one in view. When
   the frame is shorter than the form, the focused row, the error line, the
   footer, the notes, and the top band are kept in that order, so a refusal
   and the row it is about always show.

6. **Option keys by kind.** `←` and `→` step a choice or a one-of-N row,
   flip a toggle, and move a check group's own cursor; `Space`
   (`FormKey::Toggle`) flips a toggle or the item under that cursor and
   steps the others on. On a text or path field the three keys edit (move
   the text cursor, type a space). A check group of one item flips on any
   of them, which is how a single launch option behaves today.

7. **One-of-N rows have their own box.** A one-of-N option is drawn `(•)`
   when chosen and `( )` when not, the bullet mark between parentheses; a
   checkbox is `[x]`/`[ ]`. Their shapes differ, so neither needs color to
   be told apart. Both are built from glyphs already in the vetted table,
   so nothing new needs measuring.

8. **One grid.** The cursor gutter, a label column as wide as the longest
   shown label but between 11 and 16 cells (a longer label is cut with
   `…`), a two-cell gap, and the value; every kind of row starts its value
   text in one column.

9. **The host moves onto it.** The new mux form is a `Form<Msg>`: the host
   keeps the seeds, the two `required` checks (`mux: name is required`,
   `mux: cwd is required`), and the `Msg::CommitMuxNew` mapping with the
   cwd expanded; its `Enter` verb is `create`; it is opened through the
   reducer only, and no longer posts a key hint on the status line, since
   it shows its keys. The launch and pin edit forms keep their own rows
   until their stories move them, but draw them through the framework's
   row kit and `FormBody`: the launch form's footer and error line stay in
   view, its error line sits directly above the footer, its labels are
   `name`, `socket`, and `argv`, and its pin row is a one-of-N row.
   `warning_row` is deleted, and the host's `options_row` and `choice_row`
   are the framework's check and one-of-N rows.

## Consequences

- **Visible changes** (the CSP-736 summary and the changelog list them):
  `Enter` in the new mux form creates the session from the name field
  (its footer reads `Enter create`); its refusals are on its error line
  instead of a toast; opening it with `n` no longer changes the status
  line; on a short terminal the new mux and launch forms keep their keys
  in view and scroll only their rows; the launch form's error line no
  longer has a blank line between it and the keys; its labels shorten;
  its pin row draws `(•) don't pin ( ) launch, then pin ( ) pin, don't
  launch`; and on a terminal too short for the pin edit form's fixed
  lines, the focused row now shows.
- The launch form's `Tab`-only socket stop, its `LaunchSpecFormState`,
  and the pin edit form's override rows stay until CSP-737 and CSP-738
  move those forms onto `Form`; the override row is a seam on the control
  page.
- A host that keeps one field in step with another (a derived line, the
  options that follow a choice) wraps the form and reads `Form::edited`
  after each key.

## Alternatives Considered

- **Keep "Enter advances" as a per-form policy.** It keeps the new mux
  form's muscle memory, but house rule 6 settles the standard, and a policy
  flag would keep two behaviors for users to learn. Rejected.
- **The error line under the rows, before the footer, with a blank line
  between.** The launch form's old place; on a short terminal it is the
  first thing to scroll away. The footer slot keeps it next to the keys.
- **The error in place of the footer**, as the prompt does on a tiny
  screen. The form has rows to spare more often than the prompt; it shows
  both and drops rows first.
- **A single-cell radio glyph** (`●`/`○`). One cell narrower, but `●` is the
  host's agent-session glyph and `○` its open-pull-request glyph, and a new
  glyph needs measuring; `(•)` reuses the vetted bullet and reads as a
  radio button in plain text.
- **`Space` as `OptionNext`, as before.** A check group of several items
  could then not tell moving its cursor from flipping an item. A separate
  action under the same verb leaves every footer as it was.
- **Field ids as a type parameter** (`Form<M, Id>`). Typed ids, but every
  host enum of overlays would carry the parameter; `&'static str` ids keep
  `Form<M>` one type per message.
- **Labels in a fixed 11-cell column, as the launch form had.** Simpler,
  but the pin edit form needs up to 16; the clamp gives both the column
  they have today.

## Open Questions Answered

- *Which `Enter` rule?* Submit from any field.
- *One ring or two?* One, one stop per field; a control that shares a row
  with another (the launch form's socket checkbox) becomes its own field
  when that form moves.
- *Where do a form's refusals go?* One error line directly above the
  footer, until the next edit.
- *What does a one-of-N row look like?* `(•) chosen ( ) other`.
- *Which labels?* The short forms: `name`, `socket`, `argv`, `branch`.
