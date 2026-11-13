use super::*;

/// Every title is synthetic, built from the forms in Codex's source; none
/// was recorded from a running Codex.
const GOLDEN: &[(&str, TitleMark)] = &[
    // The research's table.
    ("⠋ fix-auth | conspectus", TitleMark::Working),
    (
        "[ ! ] Action Required fix-auth | conspectus",
        TitleMark::Waiting,
    ),
    (
        "[ . ] Action Required fix-auth | conspectus",
        TitleMark::Waiting,
    ),
    ("● ⠙ fix-auth | conspectus", TitleMark::Working),
    ("fix-auth | conspectus", TitleMark::Neither),
    ("", TitleMark::Neither),
    ("| fix-auth", TitleMark::Neither),
    ("build box: ~/src/conspectus", TitleMark::Neither),
    // The marks alone, and after the microphone's dot.
    ("⠏", TitleMark::Working),
    ("⠸ conspectus", TitleMark::Working),
    ("[ ! ] Action Required", TitleMark::Waiting),
    ("● [ ! ] Action Required fix-auth", TitleMark::Waiting),
    ("● [ . ] Action Required", TitleMark::Waiting),
    ("● fix-auth | conspectus", TitleMark::Neither),
    ("●", TitleMark::Neither),
    // Older defaults: the spinner and the project only.
    ("⠴ conspectus", TitleMark::Working),
    // Codex's ASCII fallbacks under GNU screen.
    ("/ fix-auth", TitleMark::Neither),
    ("- fix-auth", TitleMark::Neither),
    ("\\ fix-auth", TitleMark::Neither),
    ("* fix-auth", TitleMark::Neither),
    // tmux's default title and shells' titles.
    ("devbox.local", TitleMark::Neither),
    ("user@devbox: ~/src", TitleMark::Neither),
    // A task name that merely contains a mark is no mark.
    ("fix ⠋ spinner | conspectus", TitleMark::Neither),
    ("fix-auth ⠋ | conspectus", TitleMark::Neither),
    (
        "fix-auth [ ! ] Action Required | conspectus",
        TitleMark::Neither,
    ),
    ("review: Action Required | conspectus", TitleMark::Neither),
    ("Action Required fix-auth", TitleMark::Neither),
    ("[!] Action Required fix-auth", TitleMark::Neither),
    ("[ ! ] action required fix-auth", TitleMark::Neither),
    // A mark that runs into the next word is no mark.
    ("⠋fix-auth | conspectus", TitleMark::Neither),
    ("[ ! ] Action Requiredness", TitleMark::Neither),
    // Something before the activity hides it.
    (" ⠋ fix-auth", TitleMark::Neither),
    ("●⠋ fix-auth", TitleMark::Neither),
    // Braille that is not one of the spinner's frames.
    ("⠁ fix-auth", TitleMark::Neither),
    ("⣿ fix-auth", TitleMark::Neither),
];

#[test]
fn each_title_reads_the_mark_of_its_activity() {
    for (title, expected) in GOLDEN {
        assert_eq!(mark(title), *expected, "{title:?}");
    }
}

#[test]
fn every_spinner_frame_reads_working() {
    for frame in SPINNER_FRAMES {
        assert_eq!(
            mark(&format!("{frame} fix-auth")),
            TitleMark::Working,
            "{frame}"
        );
    }
}

#[test]
fn the_codex_adapter_classifies_its_titles() {
    use crate::discovery::harness::adapters_for_program;
    let codex = adapters_for_program("codex").next().expect("codex adapter");
    assert_eq!(
        codex.pane_title_mark("[ ! ] Action Required"),
        Some(TitleMark::Waiting)
    );
    assert_eq!(codex.pane_title_mark("fix-auth"), Some(TitleMark::Neither));
    // No other harness's titles are read: Claude Code's glyphs are not
    // recorded yet.
    for program in ["claude", "opencode", "pi", "aider"] {
        for adapter in adapters_for_program(program) {
            assert_eq!(
                adapter.pane_title_mark("⠋ fix-auth"),
                None,
                "{}",
                adapter.harness_key()
            );
        }
    }
}
