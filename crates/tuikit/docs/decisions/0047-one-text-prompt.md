# 0047: One text prompt, which explains its refusals

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/text_prompt.md](../controls/text_prompt.md) |

## Decision

- `TextPrompt<M>` has a title, an initial value with the cursor at its end,
  an optional placeholder drawn in `inert` while the field is empty, and
  `on_confirm`, an `Fn(&str) -> PromptResult<M>` shared behind an `Arc`
  (`Send + Sync`, so a prompt clones cheaply and a host's enum of overlays
  stays `Clone`). The closure answers `Commit(M)`, `Back(M)` (as a step),
  `Reject(String)` (stay open and show the reason), or `Stay`. The framework
  does not trim the text: what blank means is the host's.
- It implements `Overlay<M, Cx>` for every context and reads none. It
  resolves `TEXT_INPUT_KEYS` before the line editor: `Enter` asks
  `on_confirm`, `Esc` closes, `Ctrl-C` dismisses, both silently; `Tab` is
  swallowed; every other key edits, `q` included. It takes text, so a paste
  goes to it.
- **One error line** (house rule 7): a refusal sets one line under the
  field, in `error`, cut with `…`, which the next key that changes the text
  clears. On a screen of five rows or fewer the reason takes the footer's
  place on the bottom border, so no refusal is invisible.
- **One size.** The compact frame, as tall as its rows: the title on the top
  border, the field, and the footer generated from `TEXT_INPUT_KEYS` on the
  bottom border; four rows while an error shows.

## Rationale

The host refused input silently in places, showed reasons in three ways,
and posted prompt hints on a status line that an overlay could cover. A
prompt that shows its own keys and its own refusals needs nothing from the
host but the decision.

## Consequences

- A prompt can be a level of another overlay or a step on the stack.

## Alternatives considered

- **The footer as a body row.** A one-line prompt becomes four rows for two
  hints.
- **The hints on the status line.** The host must post and clear them, and
  a prompt that steps back leaves its hint behind.
- **The error always in place of the footer.** The keys would vanish when
  the user most needs to know how to leave; kept as the tiny-screen
  fallback.
- **Refusals as toasts.** Transient, and away from the field.
- **A generic closure parameter.** No allocation, but a host's enum of
  overlays cannot name a closure's type.
- **`on_confirm` returning `Result<M, String>`.** A host that wants to stay
  open without a reason would have to invent one.
- **The framework trimming the text.** Some values need their spaces.

## Origin

- host ADR 0166, "Text prompts are one overlay that explains its refusals":
  decisions 1 to 5.
- host ADR 0176, "Settings panels are one control that applies each change
  as it is made, and their editors are steps": decision 4 (a prompt's
  `Back`).
