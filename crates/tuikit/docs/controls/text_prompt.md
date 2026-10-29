# `TextPrompt<M>`

| | |
| --- | --- |
| Module | `tuikit::text_prompt` |
| Layer | 1 (control) |
| Type parameters | `M`, the host's message, which a confirmed text becomes; the prompt implements `Overlay<M, Cx>` for every context `Cx` and reads none |
| Cargo feature | none |
| Outcomes | `Commit(M)` when `on_confirm` takes the text, or `Back(M)` when it takes it and steps back; `Close` for `Esc`; `Dismiss` for `Ctrl-C`; `Consumed` for an edit, a swallowed `Tab`, a refusal, or a confirm that stays; never `Stay(M)` |

## Purpose

`TextPrompt<M>` asks for one line of text in a small modal: a new name, a
setting's value, a filter's text. `Enter` hands the text to a closure the
host gives it, `on_confirm`, which answers with a `PromptResult`:

- `Commit(M)`: take the text; the prompt closes and the host handles `M`.
- `Back(M)`: take the text and step back; the prompt closes as `Esc`
  closes it, so the overlay it is a step of stays open, and the host
  handles `M`.
- `Reject(String)`: refuse it and say why; the prompt stays open with the
  reason on its error line until the next edit.
- `Stay`: neither; the prompt stays open as it is.

It replaces what grows prompt by prompt: one prompt that closes on blank
text and posts a toast, another that stays open and says nothing, a third
that reports through a status line its own overlay covers; a flag on the
host's state that tells two prompts' confirms apart; a key hint composed
by hand on the status line for one prompt and none for the others.

The host keeps the title, the initial text, and what the text means: the
closure trims, parses, and checks it with the host's own parsers and builds
the host's message, or gives the reason it is refused. Two prompts that
look the same but mean different things are two closures, not one prompt
and a flag.

## Usage example

A host that names files: a blank name is refused with a reason, a name
already taken is refused with another, and a new name commits. The prompt
sits on the host's stack like any overlay.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::overlay::{ModalStack, Outcome};
use tuikit::text_prompt::{PromptResult, TextPrompt};
use tuikit::theme::Theme;

#[derive(Debug, PartialEq)]
enum Msg {
    Rename { from: String, to: String },
}

struct Files {
    names: Vec<&'static str>,
}

/// The prompt that renames `from`, refusing blank and taken names.
fn rename(from: &str, taken: Vec<String>) -> TextPrompt<Msg> {
    let from = from.to_string();
    TextPrompt::new("Rename file", from.clone(), move |text: &str| {
        let to = text.trim();
        if to.is_empty() {
            PromptResult::Reject("a name is required".into())
        } else if to != from && taken.iter().any(|name| name == to) {
            PromptResult::Reject(format!("{to} already exists"))
        } else {
            PromptResult::Commit(Msg::Rename { from: from.clone(), to: to.to_string() })
        }
    })
    .placeholder("new name")
}

let files = Files { names: vec!["notes", "plan"] };
let taken = files.names.iter().map(|name| name.to_string()).collect();
let mut stack: ModalStack<TextPrompt<Msg>> = ModalStack::new();
stack.push(rename("notes", taken));

let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
let ctrl_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
stack.route(ctrl_u, &files);
for c in "plan".chars() {
    stack.route(press(KeyCode::Char(c)), &files);
}

// A taken name is refused: the prompt stays open and says why.
let outcome = stack.route(press(KeyCode::Enter), &files).expect("a prompt is open");
assert_eq!(outcome, Outcome::Consumed);
assert_eq!(stack.top().and_then(|prompt| prompt.error()), Some("plan already exists"));

// The next edit clears the reason; a free name commits and closes.
stack.route(press(KeyCode::Char('s')), &files);
assert_eq!(stack.top().and_then(|prompt| prompt.error()), None);
let outcome = stack.route(press(KeyCode::Enter), &files).expect("a prompt is open");
let settled = stack.settle(outcome);
assert_eq!(settled.message, Some(Msg::Rename { from: "notes".into(), to: "plans".into() }));
assert!(stack.is_empty());

// Drawing: the compact frame, three rows, the footer on its border.
let theme = Theme::default();
let prompt = rename("notes", Vec::new());
let area = Rect::new(0, 0, 50, 10);
let mut buf = Buffer::empty(area);
prompt.draw(area, &mut buf, &theme);
let rect = prompt.rect(area);
assert_eq!(rect.height, 3);
let bottom: String = (rect.x..rect.right()).map(|x| buf[(x, rect.bottom() - 1)].symbol()).collect();
assert!(bottom.starts_with("└ Enter confirm · Esc cancel ─"));
```

A prompt that is a menu's follow-up question (the value a menu entry
needs) opens as a step of the menu, with `ModalStack::push_step`: `Esc`
gives the menu its keys back, and a commit or `Ctrl-C` closes the prompt
and the menu together. A prompt that edits one value of an overlay that
stays open (a settings panel's editor, see
[settings_panel.md](settings_panel.md)) opens as a step too, and its
`on_confirm` answers `PromptResult::Back`: the value applies, the prompt
closes, and the panel under it has the keys again, while `Ctrl-C` still
dismisses both. A host whose overlay holds the prompt as one of its own
levels instead keeps the prompt in its state, routes keys to it with
`Overlay::handle`, draws it with `TextPrompt::draw`, takes the prompt's
`Close` as `Esc` stepping back to its own level, and takes `Dismiss` as
`Ctrl-C` dismissing its whole overlay.

### Size

The prompt is drawn in the compact modal frame (`PROMPT_SIZE`, which is
`ModalSize::COMPACT`: up to 60 columns, at least 20, two columns clear on
each side while the screen allows) and is as tall as its rows plus the
border: **3 rows** (the title on the top border, the field, the footer on
the bottom border), and **4 rows** while an error line shows. The frame is
centered each time it is drawn, so growing moves its top up a row on most
screens. `TextPrompt::rect` is the rect it draws in.

The footer is laid out for the inner width less the space on each side of
it and breaks only between hints, so a narrow prompt keeps the hints that
fit whole (`Enter confirm` at the 20-column minimum). The error line is
cut with `…` to the inner width. A screen too short for the fourth row
(five rows or fewer) puts the reason on the bottom border in place of the
hints, so a refusal is never invisible.

## Key table

`TEXT_INPUT_KEYS`, over `TextInputKey` (a standard table; see
[keys.md](keys.md)), consulted before the line editor's
`LINE_EDIT_KEYS` (see [text_edit.md](text_edit.md)).

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the prompt (`Dismiss`), and the flow it is a step of, saying nothing; an alias, not in the footer |
| `Enter` | Confirm | `on_confirm` decides: commit, refuse with a reason, or stay |
| `Esc` | Cancel | closes the prompt (`Close`), saying nothing; as a step it gives the keys back to the overlay under it |
| `Tab` | Tab | swallowed, so a tab never reaches the text or another control |
| any other key | (line editor) | edits the line; `q` and every letter type themselves; a key that changes the text clears the error line |

The footer, generated from the table, reads `Enter confirm · Esc cancel`.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border, its title, and the footer on the bottom border |
| `surface` | the background the frame paints |
| `field_cursor` | the caret, the cell under the cursor |
| `error` | the error line (or, on a screen too short for it, the reason on the bottom border) |
| `inert` | the placeholder, while the field is empty |

The text the user types is unstyled.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 4 | Close keys | `Esc` closes the prompt's one level (as a step, back to what is under it); `Ctrl-C` dismisses it and its flow; `q` is typed, since a prompt always has text in play |
| 5 | Close feedback | closing returns `Close` or `Dismiss` and carries no message, so the host posts nothing |
| 7 | Validation errors | a refused confirm sets one error line in the footer slot under the field; the next edit clears it; no confirm is refused silently |
| 8 | Footers and hints | the footer is generated from `TEXT_INPUT_KEYS`, in the one grammar; the prompt needs no hint on the host's status line |
| 13 | Theme | the frame, caret, error line, and placeholder take theme tokens; no raw color or reversed cell |
| 14 | Width | the field windows in cells (`TextField`), and the footer and error line are cut in cells |
| 15 | Text editing | the field is a `LineEdit`, with the shared editing keys and the one-line paste rule |

Not applicable: 1, 2, 10, and 12 (a prompt has no list and does not
page), 3 (confirming the text is the prompt's one action; whether a
consequence needs a second confirmation is the host's), 6 (a form's ring
is `Form`'s), 9 (the focus cue is the pane frame's), 11 (the prompt
reports no feedback; its refusal is the error line), 16 (path completion
is `PathField`'s), 17 (titles and reasons are the host's copy), and 18
(pagers).

## Seams left open

- **Live validation.** A refusal shows only after `Enter`. A prompt that
  checks as the text changes (marking a taken name while it is typed)
  would take a second closure called on each edit, beside `on_confirm`.
- **Completion and history.** A prompt with candidates (recent values,
  paths) would draw them under the field and take `Tab` and the arrows,
  which the prompt swallows today; `PathField` brings completion for paths.
- **A minibuffer host.** The prompt draws itself in a modal frame. A host
  that wants the same contract in a one-row bar at the screen's edge would
  draw the field and the error line there; the key handling and
  `PromptResult` would not change.
- **Mouse hit testing.** `rect(area)` is the rect the prompt drew, which a
  click on or outside it would be tested against.
