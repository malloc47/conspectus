//! What a Codex pane's terminal title says about its agent (ADR 0217).
//!
//! Codex builds its title from items, by default its activity, the
//! thread's name, and the project's name. The activity item leads the
//! title and is all this module reads:
//!
//! | Activity | Title begins with | Mark |
//! | :- | :- | :- |
//! | a task runs | a braille spinner frame (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) | working |
//! | the operator is needed | `[ ! ] Action Required`, or `[ . ] Action Required` in the blink's other phase | waiting |
//! | idle | the thread or project name | neither |
//!
//! Either may follow `● `, which Codex puts first while its microphone
//! listens, and a mark counts only when the end of the title or a space
//! follows it. Everything else reads neither: an empty title, the host
//! name tmux gives a pane, a title the operator or a shell set, the
//! ASCII frames Codex writes under GNU screen (a leading `|`, `/`, `-`, or
//! `\` looks like any other title), and a title with a spinner frame or
//! the words `Action Required` anywhere but at its start. Neither is no
//! evidence: the same plain title shows while Codex is idle, while it
//! starts, and while it works with its animations off.
//!
//! The title can carry the operator's own words in the thread name, so
//! [`mark`] keeps nothing of it: it compares the title's first few
//! characters with the activity forms and returns an enum.
//!
//! The forms come from Codex's source (`codex-rs/tui`), not from a
//! recorded session.

use crate::model::live::TitleMark;

/// The braille frames of Codex's title spinner, in its order (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`),
/// spelled as escapes: they are matched here, never drawn.
pub(crate) const SPINNER_FRAMES: [char; 10] = [
    '\u{280b}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283c}', '\u{2834}', '\u{2826}', '\u{2827}',
    '\u{2807}', '\u{280f}',
];

/// The marker Codex's title leads with while the operator is needed,
/// in both phases of its blink.
const ACTION_REQUIRED: [&str; 2] = ["[ ! ] Action Required", "[ . ] Action Required"];

/// What Codex shows before the activity while its microphone listens
/// (`● `).
const LISTENING: &str = "\u{25cf} ";

/// The mark `title` carries in its activity position.
pub(crate) fn mark(title: &str) -> TitleMark {
    let activity = title.strip_prefix(LISTENING).unwrap_or(title);
    if ACTION_REQUIRED
        .iter()
        .any(|marker| leads_with(activity, marker))
    {
        return TitleMark::Waiting;
    }
    let mut chars = activity.chars();
    if chars
        .next()
        .is_some_and(|first| SPINNER_FRAMES.contains(&first))
        && ends_a_word(chars.as_str())
    {
        return TitleMark::Working;
    }
    TitleMark::Neither
}

/// Whether `text` begins with `prefix` as a whole word.
fn leads_with(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix).is_some_and(ends_a_word)
}

/// Whether `rest`, the text after a mark, leaves the mark a word of its
/// own: nothing follows it, or a space does.
fn ends_a_word(rest: &str) -> bool {
    rest.is_empty() || rest.starts_with(' ')
}

#[cfg(test)]
#[path = "codex_title_tests.rs"]
mod tests;
