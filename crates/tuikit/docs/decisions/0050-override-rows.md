# 0050: An override row follows an inherited value until it is edited

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/form.md](../controls/form.md) |

## Decision

- **A decorator, not a kind.** `Field::with_override` gives a text, path, or
  choice field an inherited value, a source word (where that value comes
  from, in the host's words), whether the field holds a value of its own as
  the form opens, and an optional hint. A toggle, check group, one-of-N
  row, or read-only line cannot follow a value.
- **Where a field stands.** `Standing` is `Follows`, `Kept`, `Changed` (a
  submit keeps the new value), or `Reset` (a submit drops the field's own
  value). Editing changes it; the reset key puts the inherited value back,
  which is `Reset` only when there was a value of its own to drop; typing
  after a reset makes the value the field's own again. `on_submit` reads
  each field's standing from the values.
- **The reset key is the form's own.** `FormKey::Inherit` is in no standard
  table; a form with override rows binds a key to it in its own table. It
  acts on the focused override row only, never reaches a field's text, and
  clears the error line when it changes something.
- **One source column.** In a form with override rows every text, path, and
  choice row ends in one right-aligned column as wide as the longest word
  any row may show: the source word while the field follows, `kept`, or,
  in `warning` while a change is pending, `will keep` or `will follow`.
  Other rows leave it blank, so every value ends in one place.
- **Explanations stay in view.** `Form::intro` is a paragraph fixed above
  the rows; `Form::details(n)` is a band of `n` lines under them showing
  the focused field's hints at full width. Both keep the form's height
  steady and give way, on a short terminal, after the focused row, the
  error line, and the footer.

## Rationale

A form that edits values layered over defaults has to show, for each value,
whether it is the field's own or follows another layer, and how to go back
to following.

## Consequences

- The words the framework writes (`kept`, `will keep`, `will follow`) are
  plain English every such form needs; the source word is the host's,
  because only the host knows where a value comes from.

## Alternatives considered

- **A field kind for overrides** wrapping another kind. Every match over
  kinds would handle the wrapper.
- **The reset key in the standard form table.** Every form would claim it,
  and a footer would name a key that does nothing.
- **Only the framework's words, or only the host's.** Neither side knows all
  four: the standings are the framework's, the source the host's.
- **A fixed summary line above the rows.** It would need a second kind of
  field; the intro is the fixed band.
- **The detail lines before the footer on a short terminal.** The keys and
  a refusal matter more than an explanation.

## Origin

- host ADR 0170, "An edit form with override rows": decisions 1 to 4.
