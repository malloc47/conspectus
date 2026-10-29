# ADR 0166: Text Prompts Are One Overlay That Explains Its Refusals

## Status

Accepted. Carries out the `TextPrompt<M>` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying
its house rules 4, 5, 7, and 8 to every one-line prompt, and finishes the
hand-off [ADR 0165](0165-tui-text-entry-one-line-editor-and-one-text-field.md)
item 7 left open: the modal prompt leaves the host. Narrows
[ADR 0030](0030-tui-text-input-primitive.md) (text-input primitive): its
prompt and the `Enter`/`Esc`/`Ctrl-C`/`Tab` contract are the framework's
`TextPrompt`, and `src/tui/widgets/input.rs` is gone. Amends
[ADR 0158](0158-tui-modals-one-frame-and-named-sizes.md) decision 4: a
frame may carry a footer on its bottom border, in the title's form.
Extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `text_prompt` module. Amended by
[ADR 0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md):
`PromptResult` gains `Back(M)`, a confirm that steps back to the overlay
the prompt is a step of.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/03-text-entry-and-forms.md`, sections
1, 2.1, 2.8, 6, and 7) found five surfaces that are one dialog: the session
and mux rename, the pin rename, the worktree branch prompt, and the
Controls max-age and contents editors. Each confirmed and refused in its
own way:

- **Five empty-input behaviors.** A session rename took blank text as
  clearing the alias; a mux rename and a pin rename closed the prompt and
  posted a toast (`mux rename: name cannot be empty`, `pin: nothing
  entered`); an empty branch name and an invalid max age kept the prompt
  open and posted a toast, an interim CSP-715 chose because the Controls
  overlay could cover the status line; a blank contents query cleared the
  search.
- **A routing flag.** Both renames opened the same prompt, whose confirm
  was always `Msg::CommitRename`. A pin rename set `App::slot_prompt` so
  the reducer would re-route the message to the slot's store write, and
  every other opener had to clear the flag, or a cancelled pin prompt
  would capture the next rename.
- **Hints in three places.** The session and mux rename composed
  `rename: Enter confirm · Esc cancel` on the status line by hand; the
  branch, max-age, and contents prompts showed no keys, and the worktree
  menu's own hint stayed on the status line while its branch prompt had
  the keys.
- **Four copies of one mapping.** The rename wrapper, the worktree menu,
  and the two Controls editors each matched the prompt's
  `InputOutcome { Continue, Confirm, Cancel, Abort }` and decided for
  themselves what a confirm meant.

## Decision

1. **`tuikit::text_prompt` owns the prompt.** Its items: `TextPrompt<M>`,
   `PromptResult<M>`, and `PROMPT_SIZE`. A prompt has a title, an initial
   value (the cursor at its end), an optional placeholder (drawn in
   `inert` while the field is empty), and `on_confirm`.

2. **One decision about the text.** `on_confirm: Fn(&str) ->
   PromptResult<M>` gets the text as typed when `Enter` is pressed and
   returns `Commit(M)` (the prompt closes and the host handles `M`),
   `Reject(String)` (the prompt stays open and shows the reason), or
   `Stay` (the prompt stays open as it is). The closure is shared behind
   an `Arc` and must be `Send + Sync`, so a prompt clones cheaply and the
   host's enum of overlays stays `Clone`. The framework does not trim:
   what blank text means is the host's.

3. **One contract.** `TextPrompt<M>` implements `Overlay<M, Cx>` for every
   `Cx` and reads no context. It resolves `TEXT_INPUT_KEYS` before the
   line editor: `Enter` asks `on_confirm`, `Esc` and `Ctrl-C` return
   `Close` with no message (house rules 4 and 5), `Tab` is swallowed, and
   every other key edits the line, `q` included. It says it takes text,
   so a paste goes to it.

4. **One error line (house rule 7).** A rejection sets one line under the
   field, in the theme's `error`, cut with `…` to the width. The next key
   that changes the text clears it; a cursor move, a swallowed `Tab`, or a
   `Stay` leaves it. On a screen too short for its row (five rows or
   fewer) the reason takes the footer's place on the bottom border, so no
   refusal is invisible.

5. **One size and a footer on the border.** The prompt is the compact
   frame, as tall as its rows and the border: three rows (the title on the
   top border, the field, the footer on the bottom border), four while an
   error line shows, re-centered when it grows. The footer is generated
   from `TEXT_INPUT_KEYS` (`Enter confirm · Esc cancel`), laid out for the
   inner width less a space on each side, and broken only between hints.
   `ModalFrame::footer` is the frame's new option for it: a line on the
   bottom border in the title's form, in the border's color unless its
   spans say otherwise. The conformance suite's footer check reads the
   bottom border as well as the body.

6. **Levels of another overlay.** On the stack, `Esc` pops the prompt and
   uncovers what is under it. A host that keeps the prompt as a level of
   one of its own overlays routes keys to it with `Overlay::handle` and
   draws it with `TextPrompt::draw`; it checks `Ctrl-C` (the `Abort` row)
   first so that dismisses its whole overlay, and takes `Close` as `Esc`
   stepping back to its own level. The worktree menu's branch prompt and
   the Controls editors are such levels until CSP-740 rebuilds the menu on
   the stack and CSP-744 builds the settings panel.

7. **The host keeps what the text means.** Conspectus's prompts are
   closures:
   - the session rename commits any text as `Msg::CommitRename` (blank
     clears the alias); a live mux's rename refuses blank text with `mux
     rename: name cannot be empty` and commits the rest the same way;
   - the pin rename refuses blank text with `pin: nothing entered` and
     commits `Msg::CommitSlotRename { slot, new_name }`, so `SlotPrompt`,
     `App::slot_prompt`, and the `CommitRename` re-route are gone, and
     with them `RenameOverlayState`;
   - the branch prompt refuses blank text with `worktree: branch name is
     required` and commits `Msg::CommitWorktreeCreate`;
   - the max-age editor is a prompt over `Option<Duration>` that parses
     with Conspectus's `parse_max_age` and refuses with `max age: <why>`,
     and the contents editor a prompt over `Option<String>`; Controls maps
     each commit to its `Msg` with the live context.
   The hand-written `rename: …` status hint and the editors' refusal
   toasts are gone; the prompt shows its keys and its refusals itself.

## Consequences

- **Visible changes** (the CSP-734 summary and the changelog list them):
  every prompt shows `Enter confirm · Esc cancel` on its bottom border,
  where the branch, max-age, and contents prompts showed no keys and the
  renames showed them on the status line (the status line no longer
  changes when a rename opens); a refused confirm explains itself on an
  error line inside the prompt, which grows one row while it shows,
  instead of a toast; an empty pin or mux rename keeps the prompt open
  with its reason instead of closing it with a toast. Cancelling still
  says nothing, and `Esc` from the branch prompt and the Controls editors
  still steps back to the menu and the panel.
- Adding a prompt is a title, an initial value, and a closure; nothing in
  the host's state or reducer tells prompts apart.
- The framework's `Form<M>`, `SettingsPanel<M>`, and `Dialog<M>` build on
  the same error-line rule and the same footer generation.
- A screen of five rows or fewer shows a refusal on the border in place
  of the hints for as long as it lasts.

## Alternatives Considered

- **The footer as a body row.** The look of the menus, but a one-line
  prompt becomes four rows, five with an error, for two hints.
- **The hints on the status line**, as the renames had them. Three rows,
  but a host must post and clear them, and a prompt that is a level of
  another overlay would leave its hint behind when `Esc` steps back.
- **The error in place of the footer on the border**, always. The prompt
  would never grow, but the keys would vanish exactly when the operator
  needs to know how to leave, and a long reason has less room there. Kept
  only as the fallback for a screen with no row to grow into.
- **Refusals as toasts**, the CSP-715 interim. A toast is transient and
  sits away from the field; house rule 7 puts the reason where the text
  is.
- **A generic closure parameter** (`TextPrompt<M, F>`). No allocation,
  but a host's enum of overlays cannot name a closure's type, so every
  prompt would need its own entry. A boxed `Fn` cannot be cloned, and an
  `Rc` is not `Send`; an `Arc` costs one allocation per prompt.
- **`on_confirm` returning `Result<M, String>`.** Simpler, but a host
  that wants to keep the prompt open without a reason (a confirm it
  handles elsewhere) would have to invent one; `Stay` says it.
- **The framework trims the text.** A session alias and a branch name
  trim, but a host may need leading spaces; trimming is the closure's.
- **Pushing the branch prompt onto the stack now.** `Esc` would pop back
  to the menu for free, but `Ctrl-C` must close the menu too (ADR 0161
  decision 8), which needs a rule for dismissing several entries at once.
  CSP-740 rebuilds the worktree menu on the stack and settles that rule
  with the dialogs it adds.

## Open Questions Answered

- *Where does a prompt show its keys?* On its bottom border, generated
  from its table.
- *What clears an error line?* The next key that changes the text.
- *Does `Esc` in a prompt inside another overlay close the overlay?* No:
  it steps back to the overlay's level; `Ctrl-C` closes the overlay.
- *Is an empty pin rename refused or closed?* Refused, in the prompt,
  with the reason the toast used to give.
