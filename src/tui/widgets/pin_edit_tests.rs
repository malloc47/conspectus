use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tuikit::testkit::keys::{ctrl, press};

use super::*;
use crate::tui::Theme;

fn field(name: &str, value: FieldValue, source: FieldSource) -> RecipeField {
    RecipeField {
        name: name.into(),
        value,
        source,
    }
}

fn text(value: &str) -> FieldValue {
    FieldValue::Text(value.into())
}

fn argv(parts: &[&str]) -> FieldValue {
    FieldValue::List(parts.iter().map(|p| (*p).to_string()).collect())
}

/// A live mux pin whose cwd is overridden.
fn mux_init() -> PinEditInit {
    PinEditInit {
        pin: "work".into(),
        kind: PinKind::Mux,
        status: "live".into(),
        live_mux: true,
        effective: vec![
            field("label", text("work"), FieldSource::Snapshot),
            field("name", text("work"), FieldSource::Live),
            field("cwd", text("/override"), FieldSource::Override),
            field("harness", text("claude-code"), FieldSource::History),
            field("argv", argv(&["claude"]), FieldSource::Default),
        ],
        overrides: SlotFields::from([("cwd".to_string(), text("/override"))]),
        inherited: vec![
            field("label", text("work"), FieldSource::Snapshot),
            field("name", text("work"), FieldSource::Live),
            field("cwd", text("/src/app"), FieldSource::Live),
            field("harness", text("claude-code"), FieldSource::History),
            field("argv", argv(&["claude"]), FieldSource::Default),
        ],
        known_pin_names: vec!["work".into(), "other".into()],
        known_harness_keys: vec!["claude-code".into(), "codex".into(), "opencode".into()],
        cwd_candidates: Vec::new(),
        defaults: None,
    }
}

fn type_str(form: &mut PinEditForm, s: &str) {
    for c in s.chars() {
        form.handle_key(press(KeyCode::Char(c)));
    }
}

fn clear_text(form: &mut PinEditForm, chars: usize) {
    for _ in 0..chars {
        form.handle_key(press(KeyCode::Backspace));
    }
}

fn focus_row(form: &mut PinEditForm, label: &str) {
    for _ in 0..20 {
        if form.focused_label() == Some(label) {
            return;
        }
        form.handle_key(press(KeyCode::Down));
    }
    panic!("no row `{label}`: {:?}", form.row_labels());
}

fn render(form: &PinEditForm) -> String {
    let area = Rect::new(0, 0, 110, 40);
    let mut buf = Buffer::empty(area);
    form.draw(area, &mut buf, &Theme::default());
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The rows are labelled in the forms' shared vocabulary, so the worktree
/// branch reads `branch` as in the launch form (it read
/// `worktree_branch`, the field's name in the pin store).
#[test]
fn it_lists_the_pin_name_the_label_and_every_overridable_field() {
    let form = PinEditForm::new(mux_init());
    let mut expected = vec!["pin name", "label"];
    expected.extend(
        PinKind::Mux
            .overridable_fields()
            .iter()
            .map(|f| label_for(f)),
    );
    assert_eq!(
        form.row_labels(),
        [
            "pin name", "label", "name", "socket", "cwd", "harness", "argv", "branch"
        ]
    );
    assert_eq!(form.row_labels(), expected);
    let frame = render(&form);
    for label in expected {
        assert!(frame.contains(label), "`{label}` missing:\n{frame}");
    }
}

#[test]
fn each_row_says_where_its_value_comes_from() {
    let frame = render(&PinEditForm::new(mux_init()));
    let row = |label: &str| {
        frame
            .lines()
            .find(|l| {
                l.contains(&format!("│  {label} "))
                    || l.contains(&format!("│{} {label} ", tuikit::glyphs::marks::POINTER))
            })
            .unwrap_or_else(|| panic!("no row {label}:\n{frame}"))
            .to_string()
    };
    assert!(row("cwd").contains("kept"), "{}", row("cwd"));
    assert!(row("harness").contains("last seen"), "{}", row("harness"));
    assert!(row("name").contains("running"), "{}", row("name"));
    assert!(row("label").contains("saved"), "{}", row("label"));
    assert!(row("argv").contains("default"), "{}", row("argv"));
    assert!(frame.contains("renames the tmux session too"), "{frame}");
}

#[test]
fn up_and_down_move_between_rows_and_wrap() {
    let mut form = PinEditForm::new(mux_init());
    assert_eq!(form.focused_label(), Some("pin name"));
    form.handle_key(press(KeyCode::Down));
    assert_eq!(form.focused_label(), Some("label"));
    form.handle_key(press(KeyCode::Up));
    form.handle_key(press(KeyCode::Up));
    assert_eq!(
        form.focused_label(),
        Some("branch"),
        "Up from the first row wraps to the last"
    );
    assert_eq!(form.focus(), Some("worktree_branch"));
}

#[test]
fn saving_without_a_change_just_closes() {
    let mut form = PinEditForm::new(mux_init());
    assert_eq!(form.handle_key(press(KeyCode::Enter)), Outcome::Close);
}

#[test]
fn editing_a_field_sets_an_override_and_leaves_the_rest_alone() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "socket");
    type_str(&mut form, "work-sock");
    let Outcome::Commit(msg) = form.handle_key(press(KeyCode::Enter)) else {
        panic!("expected a commit");
    };
    let Msg::CommitPinEdit(request) = msg else {
        panic!("wrong message");
    };
    assert_eq!(request.pin, "work");
    assert_eq!(request.rename, None);
    assert_eq!(request.set, vec![("socket".to_string(), text("work-sock"))]);
    assert!(request.unset.is_empty());
}

#[test]
fn an_argv_edit_becomes_a_list_override() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "argv");
    type_str(&mut form, " --model 'opus 4'");
    let request = form.request().unwrap();
    assert_eq!(
        request.set,
        vec![("argv".to_string(), argv(&["claude", "--model", "opus 4"]))]
    );
}

#[test]
fn inherit_drops_an_override_and_shows_the_inherited_value() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "cwd");
    form.handle_key(ctrl('r'));
    assert_eq!(form.field_text("cwd"), Some("/src/app"));
    let request = form.request().unwrap();
    assert_eq!(request.unset, vec!["cwd".to_string()]);
    assert!(request.set.is_empty());
    assert!(render(&form).contains("will follow"), "{}", render(&form));
}

#[test]
fn inheriting_a_field_that_was_never_overridden_changes_nothing() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "socket");
    type_str(&mut form, "abc");
    form.handle_key(ctrl('r'));
    let request = form.request().unwrap();
    assert!(
        request.set.is_empty() && request.unset.is_empty(),
        "{request:?}"
    );
}

#[test]
fn emptying_an_overridden_field_clears_the_override() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "cwd");
    clear_text(&mut form, "/override".len());
    let request = form.request().unwrap();
    assert_eq!(request.unset, vec!["cwd".to_string()]);
}

#[test]
fn typing_after_inherit_overrides_again() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "cwd");
    form.handle_key(ctrl('r'));
    type_str(&mut form, "/x");
    let request = form.request().unwrap();
    assert_eq!(request.set, vec![("cwd".to_string(), text("/src/app/x"))]);
    assert!(request.unset.is_empty());
}

#[test]
fn the_name_row_renames_and_refuses_taken_or_blank_names() {
    let mut form = PinEditForm::new(mux_init());
    clear_text(&mut form, 4);
    type_str(&mut form, "main");
    assert_eq!(form.request().unwrap().rename.as_deref(), Some("main"));

    clear_text(&mut form, 4);
    type_str(&mut form, "other");
    let err = form.request().unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert!(matches!(
        form.handle_key(press(KeyCode::Enter)),
        Outcome::Consumed
    ));
    assert!(form.error().is_some());

    clear_text(&mut form, 5);
    assert!(form.request().is_err(), "a blank name is refused");
}

#[test]
fn the_skip_permissions_option_toggles_the_flag_in_the_argv_row() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "harness");
    form.handle_key(press(KeyCode::Down));
    // The options row is a field of the form like any other, labelled
    // `options`; it takes no typing.
    assert_eq!(
        form.focus(),
        Some(OPTIONS),
        "the options row sits under the harness"
    );
    assert!(!form.takes_text());
    assert!(
        render(&form).contains("[ ] skip permissions"),
        "{}",
        render(&form)
    );

    form.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(
        form.field_text("argv"),
        Some("claude --dangerously-skip-permissions")
    );
    assert!(render(&form).contains("[x] skip permissions"));
    assert_eq!(
        form.request().unwrap().set,
        vec![(
            "argv".to_string(),
            argv(&["claude", "--dangerously-skip-permissions"])
        )]
    );

    form.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(form.field_text("argv"), Some("claude"));
    assert!(
        form.request().unwrap().set.is_empty(),
        "back to what it was"
    );
}

#[test]
fn a_harness_without_options_has_no_options_row() {
    let mut init = mux_init();
    init.effective[3] = field("harness", text("opencode"), FieldSource::Override);
    init.effective[4] = field("argv", argv(&["opencode"]), FieldSource::Default);
    let form = PinEditForm::new(init);
    assert!(!render(&form).contains("skip permissions"));
}

#[test]
fn cycling_the_harness_swaps_a_default_argv_and_keeps_a_custom_one() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "harness");
    form.handle_key(press(KeyCode::Right));
    assert_eq!(form.field_text("harness"), Some("codex"));
    assert_eq!(
        form.field_text("argv"),
        Some("codex"),
        "the old default went with the old harness"
    );

    focus_row(&mut form, "argv");
    type_str(&mut form, " --model gpt");
    focus_row(&mut form, "harness");
    form.handle_key(press(KeyCode::Right));
    assert_eq!(form.field_text("harness"), Some("opencode"));
    assert_eq!(
        form.field_text("argv"),
        Some("codex --model gpt"),
        "a custom argv stays"
    );
}

#[test]
fn tab_completes_a_directory_in_a_path_row_and_moves_on_elsewhere() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("alpha-dir")).unwrap();
    let mut init = mux_init();
    let typed = format!("{}/alp", dir.path().display());
    init.effective[2] = field("cwd", text(&typed), FieldSource::Override);
    let mut form = PinEditForm::new(init);
    focus_row(&mut form, "cwd");
    form.handle_key(press(KeyCode::Tab));
    assert_eq!(
        form.focused_label(),
        Some("cwd"),
        "Tab completed instead of moving"
    );
    assert_eq!(
        form.field_text("cwd"),
        Some(format!("{}/alpha-dir", dir.path().display()).as_str())
    );

    focus_row(&mut form, "socket");
    form.handle_key(press(KeyCode::Tab));
    assert_eq!(
        form.focused_label(),
        Some("cwd"),
        "Tab on a text row moves to the next row"
    );
}

#[test]
fn only_a_mux_pin_gets_the_mux_fields() {
    let init = PinEditInit {
        pin: "repo-pin".into(),
        kind: PinKind::Repo,
        status: "live".into(),
        live_mux: false,
        effective: vec![field("remote", text("git@x:y.git"), FieldSource::Live)],
        overrides: SlotFields::new(),
        inherited: vec![field("remote", text("git@x:y.git"), FieldSource::Live)],
        known_pin_names: vec![],
        known_harness_keys: vec![],
        cwd_candidates: Vec::new(),
        defaults: None,
    };
    let form = PinEditForm::new(init);
    assert_eq!(form.row_labels(), ["pin name", "label", "remote", "path"]);
    let frame = render(&form);
    assert!(
        !frame.contains("tmux session") && !frame.contains("harness"),
        "{frame}"
    );
}

#[test]
fn escape_closes_without_saving() {
    let mut form = PinEditForm::new(mux_init());
    type_str(&mut form, "x");
    assert_eq!(form.handle_key(press(KeyCode::Esc)), Outcome::Close);
}

fn render_at(form: &PinEditForm, width: u16, height: u16) -> Vec<String> {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    form.draw(area, &mut buf, &Theme::default());
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        })
        .collect()
}

/// The column the row for `label` ends at, and its last word.
fn row_end(lines: &[String], label: &str) -> (usize, String) {
    let line = lines
        .iter()
        .find(|l| {
            l.contains(&format!("│  {label} "))
                || l.contains(&format!("│{} {label} ", tuikit::glyphs::marks::POINTER))
        })
        .unwrap_or_else(|| panic!("no row {label}"));
    let inner = line.trim_end().trim_end_matches('│').trim_end();
    (
        inner.chars().count(),
        inner.split_whitespace().last().unwrap_or("").to_string(),
    )
}

#[test]
fn the_source_word_sits_in_the_same_column_on_every_row_whatever_is_selected() {
    let mut form = PinEditForm::new(mux_init());
    let mut seen = Vec::new();
    for _ in 0..8 {
        let lines = render_at(&form, 80, 40);
        let ends: Vec<_> = ["label", "name", "cwd", "harness", "argv"]
            .iter()
            .map(|label| row_end(&lines, label).0)
            .collect();
        seen.push(ends);
        form.handle_key(press(KeyCode::Down));
    }
    assert!(
        seen.windows(2).all(|pair| pair[0] == pair[1]),
        "moving the selection moved the right-hand words: {seen:?}"
    );
    assert!(
        seen[0].windows(2).all(|pair| pair[0] == pair[1]),
        "{seen:?}"
    );
}

#[test]
fn editing_a_path_never_pushes_the_source_word_out_of_the_form() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "cwd");
    type_str(
        &mut form,
        "/a/very/long/directory/name/that/goes/on/and/on/and/on/past/the/edge",
    );
    for width in [60, 80, 120] {
        let lines = render_at(&form, width, 40);
        let (_, word) = row_end(&lines, "cwd");
        assert_eq!(
            word, "keep",
            "the tag stays whole at {width} columns: {lines:#?}"
        );
        let line = lines.iter().find(|l| l.contains(" cwd ")).unwrap();
        assert!(line.contains("will keep"), "{line}");
        assert!(line.trim_end().ends_with('│'), "{line}");
    }
}

#[test]
fn a_clean_row_shows_no_stray_marker_after_its_value() {
    let form = PinEditForm::new(mux_init());
    let lines = render_at(&form, 80, 40);
    let cwd = lines.iter().find(|l| l.contains(" cwd ")).unwrap();
    let after_value = cwd.split("/override").nth(1).unwrap();
    let words: Vec<&str> = after_value
        .trim_end()
        .trim_end_matches('│')
        .split_whitespace()
        .collect();
    assert!(
        words.len() <= 2 && words.last() == Some(&"kept"),
        "only the mark and the source word follow the value: {cwd}"
    );
}

#[test]
fn the_form_explains_what_following_means_without_jargon() {
    let lines = render_at(&PinEditForm::new(mux_init()), 80, 40).join("\n");
    assert!(
        lines.contains("A blank field follows the running mux"),
        "{lines}"
    );
    for jargon in ["inherit", "slot", "snapshot"] {
        assert!(
            !lines.to_lowercase().contains(jargon),
            "`{jargon}` in:\n{lines}"
        );
    }
}

#[test]
fn the_focused_row_says_what_it_follows() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "name");
    let lines = render_at(&form, 100, 40).join("\n");
    assert!(lines.contains("following the running mux: work"), "{lines}");
    focus_row(&mut form, "cwd");
    let lines = render_at(&form, 100, 40).join("\n");
    assert!(lines.contains("kept with this pin: /override"), "{lines}");
    assert!(
        lines.contains("/src/app"),
        "what Ctrl-R goes back to: {lines}"
    );
}

/// Drawing the form reads the default argv it decided when it opened; it
/// is decided again only when the harness or the cwd changes.
#[test]
fn drawing_the_form_reads_the_default_argv_it_kept() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().display().to_string();
    let mut init = mux_init();
    init.effective[2] = field("cwd", text(&cwd), FieldSource::Override);
    init.effective[4] = field("argv", argv(&[]), FieldSource::Default);
    init.defaults = Some(crate::launch_default::LaunchDefaults::default().with_atelier(true));
    let mut form = PinEditForm::new(init);
    assert_eq!(form.argv_now(), ["claude"]);

    // The cwd becomes a workspace. Drawing (the options row reads the
    // argv) does not look at the disk again, so the kept default stands.
    std::fs::write(
        dir.path()
            .join(crate::discovery::atelier::ATELIER_CONFIG_FILENAME),
        "[workspace]\nname = \"w\"\n",
    )
    .unwrap();
    render(&form);
    render(&form);
    assert_eq!(
        form.default.source(),
        Some(crate::launch_default::DefaultSource::HarnessDefault)
    );
    assert_eq!(form.argv_now(), ["claude"]);

    // Editing the cwd decides it again.
    focus_row(&mut form, "cwd");
    type_str(&mut form, "/");
    assert_eq!(form.argv_now(), ["atelier", "exec", "claude"]);
}

#[test]
fn an_unfinished_argv_is_read_as_far_as_it_goes_and_refused_on_save() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "argv");
    type_str(&mut form, " --append 'two words");
    form.handle_key(press(KeyCode::Enter));
    assert_eq!(form.error(), Some("`argv` has an unclosed `'` quote"));

    // The option row works on what is typed so far and writes it back
    // finished, so the save then goes through.
    focus_row(&mut form, "harness");
    form.handle_key(press(KeyCode::Down));
    form.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(
        form.field_text("argv"),
        Some("claude --append 'two words' --dangerously-skip-permissions")
    );
    assert_eq!(
        form.request().unwrap().set,
        vec![(
            "argv".to_string(),
            argv(&[
                "claude",
                "--append",
                "two words",
                "--dangerously-skip-permissions"
            ])
        )]
    );
}

// ---- on the framework's form ------------------------------------------------

/// `Ctrl-R` is the form's binding for its override rows. On the pin name
/// it does nothing (it used to put back the name the form opened with),
/// and it never reaches a text field as a character.
#[test]
fn ctrl_r_acts_only_on_the_rows_that_follow_a_value() {
    let mut form = PinEditForm::new(mux_init());
    type_str(&mut form, "-x");
    form.handle_key(ctrl('r'));
    assert_eq!(form.name_text(), "work-x");

    focus_row(&mut form, "harness");
    form.handle_key(press(KeyCode::Down));
    let argv = form.field_text("argv").map(str::to_string);
    form.handle_key(ctrl('r'));
    assert_eq!(form.focus(), Some(OPTIONS));
    assert_eq!(form.field_text("argv").map(str::to_string), argv);
}

/// Resetting a field that had no override of its own leaves nothing to
/// drop, so the row says where its value comes from again (it said `will
/// follow`).
#[test]
fn resetting_a_field_without_an_override_names_its_source_again() {
    let mut form = PinEditForm::new(mux_init());
    focus_row(&mut form, "socket");
    type_str(&mut form, "abc");
    assert_eq!(row_end(&render_at(&form, 80, 40), "socket").1, "keep");
    form.handle_key(ctrl('r'));
    let (_, word) = row_end(&render_at(&form, 80, 40), "socket");
    assert_eq!(word, "unset");
}

/// A field with no override follows the value it has now, wherever that
/// comes from (here the default argv, which without the pin's overrides
/// would be nothing): its row names that source, and `Ctrl-R` after an
/// edit puts that value back.
#[test]
fn a_field_without_an_override_follows_the_value_it_has_now() {
    let mut init = mux_init();
    init.inherited.retain(|field| field.name != "argv");
    let mut form = PinEditForm::new(init);
    assert_eq!(row_end(&render_at(&form, 80, 40), "argv").1, "default");
    focus_row(&mut form, "argv");
    type_str(&mut form, " -x");
    form.handle_key(ctrl('r'));
    assert_eq!(form.field_text("argv"), Some("claude"));
    assert_eq!(row_end(&render_at(&form, 80, 40), "argv").1, "default");
    let request = form.request().unwrap();
    assert!(request.set.is_empty() && request.unset.is_empty());
}

/// A refused save says why on the error line directly above the keys and
/// moves the focus to the field it is about (it stayed where `Enter` was
/// pressed).
#[test]
fn a_refused_save_takes_the_focus_to_the_field_it_is_about() {
    let mut form = PinEditForm::new(mux_init());
    clear_text(&mut form, 4);
    type_str(&mut form, "other");
    focus_row(&mut form, "argv");
    assert_eq!(form.handle_key(press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(form.error(), Some("a pin named `other` already exists"));
    assert_eq!(form.focus(), Some(PIN));

    let lines = render_at(&form, 80, 40);
    let at = |needle: &str| {
        lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no {needle}:\n{}", lines.join("\n")))
    };
    assert_eq!(
        at("already exists") + 1,
        at("↑/↓ move"),
        "the error line is the footer slot"
    );
    type_str(&mut form, "s");
    assert_eq!(form.error(), None, "the next edit clears it");
}

/// A field that keeps a value with nothing under it to follow says what
/// `Ctrl-R` does there.
#[test]
fn a_kept_value_with_nothing_to_follow_says_ctrl_r_drops_it() {
    let mut init = mux_init();
    init.effective
        .push(field("socket", text("work"), FieldSource::Override));
    init.overrides.insert("socket".into(), text("work"));
    let mut form = PinEditForm::new(init);
    focus_row(&mut form, "socket");
    let lines = render_at(&form, 100, 40).join("\n");
    assert!(lines.contains("kept with this pin: work"), "{lines}");
    assert!(
        lines.contains("Ctrl-R drops it; there is nothing to follow yet"),
        "{lines}"
    );
    form.handle_key(ctrl('r'));
    assert_eq!(form.request().unwrap().unset, ["socket"]);
}

#[test]
fn a_session_pin_names_its_resume_field_in_plain_words() {
    let init = PinEditInit {
        pin: "parser".into(),
        kind: PinKind::Session,
        status: "live".into(),
        live_mux: false,
        effective: vec![field("label", text("parser"), FieldSource::Snapshot)],
        overrides: SlotFields::new(),
        inherited: vec![field("label", text("parser"), FieldSource::Snapshot)],
        known_pin_names: vec!["parser".into()],
        known_harness_keys: vec![],
        cwd_candidates: Vec::new(),
        defaults: None,
    };
    let mut form = PinEditForm::new(init);
    assert_eq!(form.row_labels(), ["pin name", "label", "resume in"]);
    focus_row(&mut form, "resume in");
    type_str(&mut form, "work");
    assert_eq!(
        form.request().unwrap().set,
        vec![("resume_into".to_string(), text("work"))]
    );
}

/// Every text row ends in one column, the pin name's too (its source
/// column is blank), so the values all end in one place.
#[test]
fn the_pin_name_row_ends_where_the_override_rows_do() {
    let mut form = PinEditForm::new(mux_init());
    type_str(&mut form, &"-long".repeat(20));
    form.handle_key(press(KeyCode::Home));
    let lines = render_at(&form, 80, 40);
    let name = lines
        .iter()
        .find(|line| line.contains("pin name"))
        .expect("the pin name row");
    let mark = name
        .chars()
        .position(|c| c == '>')
        .expect("the scroll mark");
    let (end, _) = row_end(&lines, "label");
    assert_eq!(
        mark + 1 + 1 + "will follow".len(),
        end,
        "the value ends one gap and the source column before the edge"
    );
}
